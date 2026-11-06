//! Real local BDK graph/signature fixtures. No RPC, funding or settlement.
use super::super::safe_bitcoin::tests::{coin, wallet};
use super::*;

pub(in crate::wallet) fn vector(count: usize) -> (Wallet, BitcoinRecoveryRequest, u64) {
    let mut w = wallet();
    let mut points = (0..count)
        .map(|i| {
            coin(
                &mut w,
                if i % 2 == 0 {
                    KeychainKind::Internal
                } else {
                    KeychainKind::External
                },
                10000,
                true,
                false,
            )
        })
        .collect::<Vec<_>>();
    points.sort_by_key(ToString::to_string);
    let mut tx = BdkTransaction {
        version: bitcoin::transaction::Version::TWO,
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: points
            .iter()
            .map(|p| bitcoin::TxIn {
                previous_output: *p,
                sequence: bitcoin::Sequence::MAX,
                ..Default::default()
            })
            .collect(),
        output: vec![TxOut {
            value: BdkAmount::from_sat(330),
            script_pubkey: w
                ._get_new_address(KeychainKind::Internal)
                .unwrap()
                .script_pubkey(),
        }],
    };
    let vbytes =
        (bitcoin::consensus::serialize(&tx).len() as u64 * 4 + 2 + 67 * count as u64).div_ceil(4);
    tx.output[0].value = BdkAmount::from_sat(count as u64 * 10000 - vbytes * 2);
    let mut psbt = Psbt::from_unsigned_tx(tx).unwrap();
    for (i, p) in points.iter().enumerate() {
        psbt.inputs[i].witness_utxo = Some(w.bdk_wallet.get_utxo(*p).unwrap().txout);
    }
    let now = super::super::safe_bitcoin::unix_seconds().unwrap();
    (
        w,
        BitcoinRecoveryRequest {
            version: 1,
            network: "regtest".into(),
            psbt: psbt.to_string(),
            fee_rate: "2".into(),
            maximum_fee_sats: "10000".into(),
            expires_at: (now + 120).to_string(),
            input_preparation: None,
        },
        now,
    )
}
fn retain(w: &mut Wallet, r: &BitcoinRecoveryRequest) -> String {
    let txid = Psbt::from_str(&r.psbt)
        .unwrap()
        .unsigned_tx
        .compute_txid()
        .to_string();
    w.transfer_artifacts.insert(
        txid.clone(),
        TransferArtifacts {
            bitcoin_recovery: Some(BitcoinRecoveryRecord {
                request: r.clone(),
                attempted: true,
                broadcast_attempted: false,
                cancelled: false,
                restored: false,
            }),
            ..Default::default()
        },
    );
    txid
}
#[test]
fn exact_one_two_and_thirty_two_inputs_sign_and_verify_with_real_bdk() {
    for n in [1, 2, 32] {
        let (mut w, r, now) = vector(n);
        let before = serde_json::to_value(&w.transfer_artifacts).unwrap();
        w.bitcoin_recovery_preflight(&r, false, now).unwrap();
        assert_eq!(serde_json::to_value(&w.transfer_artifacts).unwrap(), before);
        let id = retain(&mut w, &r);
        assert!(w.bitcoin_spend_view().unwrap().inputs.is_empty());
        assert!(w.bitcoin_partial_input_view(&id).unwrap().inputs.is_empty());
        assert_eq!(w.managed_rgb_input_holds(None).unwrap().len(), n);
        assert!(w.sign_psbt(r.psbt.clone(), None).is_err());
        let mut psbt = w.bitcoin_recovery_preflight(&r, true, now).unwrap();
        w._sign_psbt(&mut psbt, None).unwrap();
        verify_original(
            w.transfer_artifacts[&id].bitcoin_recovery.as_ref().unwrap(),
            &psbt.to_string(),
        )
        .unwrap();
        w.transfer_artifacts.get_mut(&id).unwrap().signed_psbt = Some(psbt.to_string());
        let data = w.bitcoin_recovery_data(id.clone()).unwrap();
        assert_eq!(data["signed_psbt"], psbt.to_string());
        assert_eq!(data["own_signatures_complete"], true);
        assert_eq!(
            data["scope"],
            "RETAINED_BITCOIN_SELF_RECOVERY_NOT_SETTLEMENT"
        );
        assert!(w.bitcoin_recovery_preflight(&r, false, now).is_err());
        assert!(w.bitcoin_recovery_preflight(&r, true, now).is_err());
    }
}
#[test]
fn refuses_replacement_expiry_network_cost_or_restored_signing_and_preserves_original_reads() {
    let (mut w, r, now) = vector(2);
    for (fee, cap, expiry, network) in [
        ("0", "10000", now + 120, "regtest"),
        ("1.5", "10000", now + 120, "regtest"),
        ("2", "10001", now + 120, "regtest"),
        ("2", "1", now + 120, "regtest"),
        ("2", "10000", now, "regtest"),
        ("2", "10000", now + 121, "regtest"),
        ("2", "10000", now + 120, "mainnet"),
    ] {
        let mut changed = r.clone();
        changed.fee_rate = fee.into();
        changed.maximum_fee_sats = cap.into();
        changed.expires_at = expiry.to_string();
        changed.network = network.into();
        assert!(w.bitcoin_recovery_preflight(&changed, false, now).is_err());
    }
    let id = retain(&mut w, &r);
    assert!(w.bitcoin_recovery_preflight(&r, false, now).is_err());
    w.transfer_artifacts
        .get_mut(&id)
        .unwrap()
        .bitcoin_recovery
        .as_mut()
        .unwrap()
        .restored = true;
    assert!(w.bitcoin_recovery_preflight(&r, true, now).is_err());
    let data = w.bitcoin_recovery_data(id.clone()).unwrap();
    assert_eq!(data["restored"], true);
    assert_eq!(data["own_signatures_complete"], false);
    assert!(data["signed_psbt"].is_null());
    let mut changed = r.clone();
    changed.expires_at = (now + 121).to_string();
    assert_ne!(data["request"]["expires_at"], changed.expires_at);
}
#[test]
fn own_exemption_cannot_spend_another_retained_native_original() {
    let (mut w, r, now) = vector(2);
    let id = retain(&mut w, &r);
    let mut other = r.clone();
    let mut psbt = Psbt::from_str(&other.psbt).unwrap();
    psbt.unsigned_tx.output[0].script_pubkey = w
        ._get_new_address(KeychainKind::Internal)
        .unwrap()
        .script_pubkey();
    other.psbt = psbt.to_string();
    let other_id = retain(&mut w, &other);
    assert_ne!(id, other_id);
    assert!(w.bitcoin_recovery_preflight(&r, true, now).is_err());
    assert!(w.bitcoin_recovery_preflight(&other, true, now).is_err());
}
#[test]
fn refuses_every_foreign_changed_missing_duplicate_reordered_or_colored_return() {
    let (mut w, r, now) = vector(2);
    for mutation in 0..7 {
        let mut psbt = Psbt::from_str(&r.psbt).unwrap();
        match mutation {
            0 => psbt.inputs[1].witness_utxo.as_mut().unwrap().value = BdkAmount::from_sat(9999),
            1 => {
                psbt.inputs[1].witness_utxo.as_mut().unwrap().script_pubkey = wallet()
                    ._get_new_address(KeychainKind::Internal)
                    .unwrap()
                    .script_pubkey()
            }
            2 => {
                psbt.inputs.pop();
                psbt.unsigned_tx.input.pop();
            }
            3 => {
                psbt.inputs.swap(0, 1);
                psbt.unsigned_tx.input.swap(0, 1);
            }
            4 => {
                psbt.unsigned_tx.input[1].previous_output =
                    psbt.unsigned_tx.input[0].previous_output
            }
            5 => {
                psbt.unsigned_tx.output[0].script_pubkey = w
                    ._get_new_address(KeychainKind::External)
                    .unwrap()
                    .script_pubkey()
            }
            _ => {
                psbt.unsigned_tx.output[0].script_pubkey = wallet()
                    ._get_new_address(KeychainKind::Internal)
                    .unwrap()
                    .script_pubkey()
            }
        }
        let mut changed = r.clone();
        changed.psbt = psbt.to_string();
        assert!(
            w.bitcoin_recovery_preflight(&changed, false, now).is_err(),
            "mutation{mutation}"
        );
    }
}
#[test]
fn rejects_missing_or_invalid_each_signature_and_immutable_output_metadata() {
    let (mut w, r, now) = vector(2);
    let id = retain(&mut w, &r);
    let mut signed = w.bitcoin_recovery_preflight(&r, true, now).unwrap();
    w._sign_psbt(&mut signed, None).unwrap();
    let record = w.transfer_artifacts[&id].bitcoin_recovery.as_ref().unwrap();
    for i in 0..2 {
        let mut changed = signed.clone();
        changed.inputs[i].final_script_witness = None;
        assert!(verify_original(record, &changed.to_string()).is_err());
        let mut changed = signed.clone();
        changed.inputs[i].final_script_witness =
            Some(bitcoin::Witness::from_slice(&[vec![1u8; 64]]));
        assert!(verify_original(record, &changed.to_string()).is_err());
    }
    let mut changed = signed.clone();
    changed.outputs[0].redeem_script = Some(ScriptBuf::new());
    assert!(verify_original(record, &changed.to_string()).is_err());
    assert!(verify_original(record, &r.psbt).is_err());
}

#[test]
fn dispatch_requires_exact_retained_signature_one_native_attempt_and_live_copy() {
    let (mut w, r, now) = vector(2);
    let id = retain(&mut w, &r);
    assert!(w.bitcoin_recovery_broadcast_preflight(&id, false).is_err());
    let mut psbt = w.bitcoin_recovery_preflight(&r, true, now).unwrap();
    w._sign_psbt(&mut psbt, None).unwrap();
    w.transfer_artifacts.get_mut(&id).unwrap().signed_psbt = Some(psbt.to_string());
    w.bitcoin_recovery_broadcast_preflight(&id, false).unwrap();
    assert!(w.bitcoin_recovery_broadcast_preflight(&id, true).is_err());
    w.transfer_artifacts
        .get_mut(&id)
        .unwrap()
        .bitcoin_recovery
        .as_mut()
        .unwrap()
        .broadcast_attempted = true;
    assert!(w.bitcoin_recovery_broadcast_preflight(&id, false).is_err());
    w.bitcoin_recovery_broadcast_preflight(&id, true).unwrap();
    w.transfer_artifacts
        .get_mut(&id)
        .unwrap()
        .bitcoin_recovery
        .as_mut()
        .unwrap()
        .restored = true;
    assert!(w.bitcoin_recovery_broadcast_preflight(&id, true).is_err());
    assert_eq!(
        w.bitcoin_recovery_data(id).unwrap()["own_signatures_complete"],
        true
    );
}
