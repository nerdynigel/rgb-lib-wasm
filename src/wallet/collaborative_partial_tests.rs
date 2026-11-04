//! Real BDK/Stock local graph vectors. Never remote funding or settlement.
use super::super::safe_bitcoin::tests::{coin, wallet};
use super::*;

pub(in crate::wallet) fn vector() -> (Wallet, CollaborativePartialRequest) {
    let mut wallet = wallet();
    let a = coin(&mut wallet, KeychainKind::Internal, 10000, true, false);
    let b = coin(&mut wallet, KeychainKind::External, 20000, true, false);
    let mut other = super::super::safe_bitcoin::tests::wallet();
    let c = coin(&mut other, KeychainKind::Internal, 30000, true, false);
    let d = coin(&mut other, KeychainKind::External, 40000, true, false);
    let all = [(a, &wallet), (b, &wallet), (c, &other), (d, &other)];
    let tx = BdkTransaction {
        version: bitcoin::transaction::Version::TWO,
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: all
            .iter()
            .map(|(p, _)| bitcoin::TxIn {
                previous_output: *p,
                script_sig: ScriptBuf::new(),
                sequence: bitcoin::Sequence::MAX,
                witness: bitcoin::Witness::default(),
            })
            .collect(),
        output: vec![bitcoin::TxOut {
            value: BdkAmount::from_sat(99000),
            script_pubkey: wallet.bdk_wallet.get_utxo(a).unwrap().txout.script_pubkey,
        }],
    };
    let mut psbt = Psbt::from_unsigned_tx(tx).unwrap();
    for (i, (p, w)) in all.iter().enumerate() {
        psbt.inputs[i].witness_utxo = Some(w.bdk_wallet.get_utxo(*p).unwrap().txout);
        psbt.inputs[i].non_witness_utxo = Some(
            w.bdk_wallet
                .get_tx(p.txid)
                .unwrap()
                .tx_node
                .tx
                .as_ref()
                .clone(),
        );
    }
    let mut own = vec![a.to_string(), b.to_string()];
    own.sort();
    let request = CollaborativePartialRequest {
        version: 1,
        network: "regtest".into(),
        psbt: psbt.to_string(),
        own_outpoints: own,
        expires_at: (super::super::safe_bitcoin::unix_seconds().unwrap() + 120).to_string(),
    };
    (wallet, request)
}

#[test]
fn actual_two_keychain_inputs_sign_all_own_and_no_foreign_inputs() {
    let (w, r) = vector();
    let mut psbt = w
        .collaborative_partial_preflight(
            &r,
            false,
            super::super::safe_bitcoin::unix_seconds().unwrap(),
        )
        .unwrap();
    w.sign_collaborative_owned_inputs(&r, &mut psbt).unwrap();
    verify_partial(&r, &psbt).unwrap();
    assert!(psbt.inputs[0].tap_key_sig.is_some() && psbt.inputs[1].tap_key_sig.is_some());
    assert!(psbt.inputs[2].tap_key_sig.is_none() && psbt.inputs[3].tap_key_sig.is_none());
}

#[test]
fn one_attempt_and_original_bytes_survive_serialization_without_new_signature() {
    let (mut w, r) = vector();
    let id = Psbt::from_str(&r.psbt)
        .unwrap()
        .unsigned_tx
        .compute_txid()
        .to_string();
    w.transfer_artifacts
        .entry(id.clone())
        .or_default()
        .partial_signing = Some(PartialSigningRecord {
        request: r.clone(),
        attempted: true,
        restored: false,
    });
    assert!(
        w.collaborative_partial_preflight(
            &r,
            false,
            super::super::safe_bitcoin::unix_seconds().unwrap()
        )
        .is_err()
    );
    let mut original = w
        .collaborative_partial_preflight(
            &r,
            true,
            super::super::safe_bitcoin::unix_seconds().unwrap(),
        )
        .unwrap();
    w.sign_collaborative_owned_inputs(&r, &mut original)
        .unwrap();
    let bytes = original.to_string();
    w.transfer_artifacts.get_mut(&id).unwrap().signed_psbt = Some(bytes.clone());
    let serialized = serde_json::to_vec(&w.transfer_artifacts).unwrap();
    w.transfer_artifacts = serde_json::from_slice(&serialized).unwrap();
    let data = w.collaborative_partial_data(id.clone()).unwrap();
    assert_eq!(data["own_signatures_complete"], true);
    assert_eq!(data["signed_psbt"], bytes);
    assert!(
        w.collaborative_partial_preflight(
            &r,
            true,
            super::super::safe_bitcoin::unix_seconds().unwrap()
        )
        .is_err()
    );
    assert!(w.sign_psbt(r.psbt.clone(), None).is_err());
    assert!(w.bitcoin_spend_view().unwrap().inputs.is_empty());
}

#[test]
fn wrong_network_expiry_missing_or_foreign_owned_set_never_signs() {
    let (w, r) = vector();
    let now = super::super::safe_bitcoin::unix_seconds().unwrap();
    for n in 0..6 {
        let mut bad = r.clone();
        match n {
            0 => bad.network = "mainnet".into(),
            1 => bad.expires_at = now.to_string(),
            2 => bad.expires_at = (now + 301).to_string(),
            3 => bad.own_outpoints.pop().map(|_| ()).unwrap(),
            4 => bad.own_outpoints.push("f".repeat(64) + ":0"),
            _ => bad.own_outpoints.reverse(),
        }
        assert!(w.collaborative_partial_preflight(&bad, false, now).is_err());
    }
}

#[test]
fn read_only_view_reports_actual_native_values_and_outputs_without_mutation() {
    let (w, r) = vector();
    let before = serde_json::to_vec(&w.database).unwrap();
    let records = serde_json::to_vec(&w.transfer_artifacts).unwrap();
    let sequence = w.idb_sequence.load(std::sync::atomic::Ordering::SeqCst);
    let view = w.review_collaborative_partial(r.clone()).unwrap();
    assert_eq!(view["kind"], "wallet-collaborative-inputs");
    assert!(view["contract_id"].is_null());
    let rows = view["inputs"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    for row in rows {
        let point = OutPoint::from_str(row["outpoint"].as_str().unwrap()).unwrap();
        let output = w.bdk_wallet.get_utxo(point).unwrap();
        assert_eq!(row["input_sats"], output.txout.value.to_sat().to_string());
        assert_eq!(
            row["script_hex"],
            output.txout.script_pubkey.to_hex_string()
        );
        assert_eq!(row["rgb_units"], "0");
    }
    assert_eq!(view["owned_output_scripts"].as_array().unwrap().len(), 1);
    assert_eq!(before, serde_json::to_vec(&w.database).unwrap());
    assert_eq!(records, serde_json::to_vec(&w.transfer_artifacts).unwrap());
    assert_eq!(
        sequence,
        w.idb_sequence.load(std::sync::atomic::Ordering::SeqCst)
    );
}

#[test]
fn foreign_derivation_hints_cannot_trigger_or_publish_foreign_signatures() {
    let (w, mut r) = vector();
    let mut known = Psbt::from_str(&r.psbt).unwrap();
    w.sign_collaborative_owned_inputs(&r, &mut known).unwrap();
    let mut original = Psbt::from_str(&r.psbt).unwrap();
    original.inputs[2].tap_internal_key = known.inputs[0].tap_internal_key;
    original.inputs[2].tap_key_origins = known.inputs[0].tap_key_origins.clone();
    r.psbt = original.to_string();
    w.collaborative_partial_preflight(
        &r,
        false,
        super::super::safe_bitcoin::unix_seconds().unwrap(),
    )
    .unwrap();
    let foreign = original.inputs[2].clone();
    w.sign_collaborative_owned_inputs(&r, &mut original)
        .unwrap();
    assert_eq!(original.inputs[2], foreign);
    assert!(original.inputs[2].tap_key_sig.is_none());
    verify_partial(&r, &original).unwrap();
}

#[test]
fn changed_values_nonbinding_sighashes_and_missing_previous_transactions_refuse_before_attempt() {
    let (w, r) = vector();
    let now = super::super::safe_bitcoin::unix_seconds().unwrap();
    for n in 0..7 {
        let mut bad = r.clone();
        let mut tx = Psbt::from_str(&bad.psbt).unwrap();
        match n {
            0 => tx.inputs[0].witness_utxo.as_mut().unwrap().value = BdkAmount::from_sat(10001),
            1 => tx.inputs[0].sighash_type = Some(bitcoin::psbt::PsbtSighashType::from_u32(2)),
            2 => tx.inputs[1].sighash_type = Some(bitcoin::psbt::PsbtSighashType::from_u32(0x81)),
            3 => tx.inputs[2].non_witness_utxo = None,
            4 => tx.unsigned_tx.input[0].sequence = bitcoin::Sequence::ENABLE_RBF_NO_LOCKTIME,
            5 => tx.unsigned_tx.output[0].value = BdkAmount::from_sat(100001),
            _ => tx.unsigned_tx.output[0].value = BdkAmount::from_sat(1),
        }
        bad.psbt = tx.to_string();
        assert!(w.collaborative_partial_preflight(&bad, false, now).is_err());
        assert!(w.transfer_artifacts.is_empty());
    }
}

#[test]
fn every_own_signature_and_unchanged_foreign_maps_are_required_for_complete_original() {
    let (w, r) = vector();
    let mut signed = Psbt::from_str(&r.psbt).unwrap();
    w.sign_collaborative_owned_inputs(&r, &mut signed).unwrap();
    for n in 0..5 {
        let mut bad = signed.clone();
        match n {
            0 => bad.inputs[1].tap_key_sig = None,
            1 => bad.inputs[0].tap_key_sig = bad.inputs[1].tap_key_sig,
            2 => bad.inputs[2].tap_key_sig = bad.inputs[0].tap_key_sig,
            3 => bad.inputs[0].witness_utxo.as_mut().unwrap().value = BdkAmount::from_sat(1),
            _ => bad.unsigned_tx.output[0].value = BdkAmount::from_sat(98999),
        }
        assert!(verify_partial(&r, &bad).is_err());
    }
}

#[test]
fn retained_attempt_blocks_replacements_and_restored_attempt_never_reenters_signer() {
    let (mut w, r) = vector();
    let mut tx = Psbt::from_str(&r.psbt).unwrap();
    let id = tx.unsigned_tx.compute_txid().to_string();
    let now = super::super::safe_bitcoin::unix_seconds().unwrap();
    w.transfer_artifacts
        .entry(id.clone())
        .or_default()
        .partial_signing = Some(PartialSigningRecord {
        request: r.clone(),
        attempted: true,
        restored: true,
    });
    assert!(w.collaborative_partial_preflight(&r, true, now).is_err());
    assert!(w.collaborative_partial_preflight(&r, false, now).is_err());
    tx.unsigned_tx.output[0].value = BdkAmount::from_sat(98000);
    let mut replacement = r.clone();
    replacement.psbt = tx.to_string();
    assert!(
        w.collaborative_partial_preflight(&replacement, false, now)
            .is_err()
    );
    let original = w.collaborative_partial_data(id).unwrap();
    assert_eq!(original["restored"], true);
    assert_eq!(original["own_signatures_complete"], false);
    assert!(original["signed_psbt"].is_null());
}

#[test]
fn hidden_rgb_stock_and_unconfirmed_inputs_are_not_available_to_buyer_partial() {
    let (mut w, r) = vector();
    let point = OutPoint::from_str(&r.own_outpoints[0]).unwrap();
    super::super::safe_bitcoin::tests::assign_stock(&w, point, false);
    assert!(
        w.collaborative_partial_preflight(
            &r,
            false,
            super::super::safe_bitcoin::unix_seconds().unwrap()
        )
        .is_err()
    );
    assert!(w.transfer_artifacts.is_empty());
    let pending = coin(&mut w, KeychainKind::Internal, 50000, false, false);
    let mut bad = r.clone();
    let mut tx = Psbt::from_str(&bad.psbt).unwrap();
    tx.unsigned_tx.input[0].previous_output = pending;
    tx.inputs[0].witness_utxo = Some(w.bdk_wallet.get_utxo(pending).unwrap().txout);
    tx.inputs[0].non_witness_utxo = Some(
        w.bdk_wallet
            .get_tx(pending.txid)
            .unwrap()
            .tx_node
            .tx
            .as_ref()
            .clone(),
    );
    bad.own_outpoints = vec![
        pending.to_string(),
        tx.unsigned_tx.input[1].previous_output.to_string(),
    ];
    bad.own_outpoints.sort();
    bad.psbt = tx.to_string();
    assert!(
        w.collaborative_partial_preflight(
            &bad,
            false,
            super::super::safe_bitcoin::unix_seconds().unwrap()
        )
        .is_err()
    );
}
