//! Real BDK signatures/Stock on an invented native graph. No valid RGB original
//! consignment, live RPC, funding, delivery or broadcast: these are safety tests.
use super::super::safe_bitcoin::tests::{assign_stock, coin, retain_external, wallet};
use super::*;

fn fixture() -> (Wallet, RgbFeeRecoveryRequest, OutPoint, String) {
    let mut w = wallet();
    let fee = coin(&mut w, KeychainKind::Internal, 50_000, true, false);
    let asset = coin(&mut w, KeychainKind::External, 1000, true, false);
    assign_stock(&w, asset, false);
    let destination = w._get_new_address(KeychainKind::Internal).unwrap();
    let carrier = w._get_new_address(KeychainKind::External).unwrap();
    let mut builder = w.bdk_wallet.build_tx();
    builder
        .add_utxos(&[fee, asset])
        .unwrap()
        .manually_selected_only()
        .add_recipient(carrier.script_pubkey(), BdkAmount::from_sat(1000))
        .add_recipient(destination.script_pubkey(), BdkAmount::from_sat(49_000))
        .fee_absolute(BdkAmount::from_sat(1000));
    let original = builder.finish().unwrap();
    let (txid, _) = retain_external(&mut w, &original, true);
    let operation_id = "01a19805-0462-759b-9050-b9b0feb9da8f".to_string();
    let request = WalletRgbSendRequest {
        operation_id: operation_id.clone(),
        asset_id: "unit-reservation-graph-not-an-RGB-proof".into(),
        recipient_id: "bcrt:wvout:unit".into(),
        amount: 2,
        witness_sats: 1000,
        blinding: None,
        transport_endpoints: vec![],
        fee_rate: 1,
        min_confirmations: 1,
        excluded_inputs: vec![],
        expires_at: 1,
    };
    let mut signed = original;
    w._sign_psbt(&mut signed, None).unwrap();
    let artifacts = w.transfer_artifacts.get_mut(&txid).unwrap();
    artifacts.signed_psbt = Some(signed.to_string());
    let binding = artifacts
        .external_send
        .as_mut()
        .unwrap()
        .wallet_transfer
        .as_mut()
        .unwrap();
    binding.request_json = serde_json::to_string(&request).unwrap();
    binding.signing_attempted = true;
    (
        w,
        RgbFeeRecoveryRequest {
            version: 1,
            operation_id,
            outpoint: fee.to_string(),
            fee_sats: "1000".into(),
            expires_at: (now_seconds().unwrap() + 60).to_string(),
            excluded_inputs: vec![],
        },
        asset,
        txid,
    )
}

#[test]
fn exact_fee_input_self_payment_preserves_original_and_all_native_holds() {
    let (mut w, request, _, original) = fixture();
    let before = w
        .wallet_rgb_send_data(request.operation_id.clone())
        .unwrap();
    assert_eq!(w.bitcoin_spend_view().unwrap().total_sats, "0");
    let prepared = w.build_rgb_fee_recovery(&request).unwrap();
    assert_eq!(prepared.original_txid, original);
    assert_eq!(prepared.input_sats, "50000");
    assert_eq!(prepared.amount_sats, "49000");
    let psbt = w
        .checked_rgb_fee_recovery_psbt(&request, &prepared.psbt, false)
        .unwrap();
    assert_eq!(psbt.inputs.len(), 1);
    assert_eq!(psbt.unsigned_tx.output.len(), 1);
    assert!(matches!(
        w.bdk_wallet
            .derivation_of_spk(psbt.unsigned_tx.output[0].script_pubkey.clone()),
        Some((KeychainKind::Internal, _))
    ));
    assert_eq!(
        w.wallet_rgb_send_data(request.operation_id).unwrap(),
        before
    );
    assert_eq!(w.bitcoin_spend_view().unwrap().total_sats, "0");
    assert!(
        w.sign_psbt(prepared.psbt, None).is_err(),
        "the general signer remains fenced"
    );
}

#[test]
fn established_bdk_signer_signs_only_the_new_separately_approved_self_payment() {
    let (mut w, request, _, original) = fixture();
    let old_signed = w.transfer_artifacts[&original].signed_psbt.clone();
    let prepared = w.build_rgb_fee_recovery(&request).unwrap();
    let mut psbt = w
        .checked_rgb_fee_recovery_psbt(&request, &prepared.psbt, false)
        .unwrap();
    w._sign_psbt(&mut psbt, None).unwrap();
    let result = w
        .checked_rgb_fee_recovery_psbt(&request, &psbt.to_string(), true)
        .unwrap();
    let tx = result.extract_tx().unwrap();
    assert_ne!(tx.compute_txid().to_string(), original);
    assert_eq!(tx.input[0].witness.len(), 1);
    assert_eq!(tx.input[0].witness.iter().next().unwrap().len(), 64);
    assert_eq!(w.transfer_artifacts[&original].signed_psbt, old_signed);
    assert!(
        !w.transfer_artifacts[&original]
            .external_send
            .as_ref()
            .unwrap()
            .wallet_transfer
            .as_ref()
            .unwrap()
            .broadcast_attempted
    );
}

#[test]
fn rgb_seals_foreign_inputs_and_unrelated_operations_are_never_recovery_coins() {
    let (mut w, request, asset, _) = fixture();
    for change in [asset.to_string(), OutPoint::null().to_string()] {
        let mut bad = request.clone();
        bad.outpoint = change;
        assert!(w.build_rgb_fee_recovery(&bad).is_err());
    }
    let mut bad = request;
    bad.operation_id = "00000000-0000-4000-8000-000000000000".into();
    assert!(w.build_rgb_fee_recovery(&bad).is_err());
}

#[test]
fn restored_dispatched_unsigned_and_nonprepared_originals_fail_closed() {
    for mode in [
        "restored",
        "dispatched",
        "unsigned",
        "not_sign_attempted",
        "finalized",
        "cancelled",
        "conflicted",
    ] {
        let (mut w, request, _, original) = fixture();
        let artifact = w.transfer_artifacts.get_mut(&original).unwrap();
        let external = artifact.external_send.as_mut().unwrap();
        match mode {
            "restored" => external.wallet_transfer.as_mut().unwrap().restored = true,
            "dispatched" => {
                external
                    .wallet_transfer
                    .as_mut()
                    .unwrap()
                    .broadcast_attempted = true
            }
            "unsigned" => artifact.signed_psbt = None,
            "not_sign_attempted" => {
                external.wallet_transfer.as_mut().unwrap().signing_attempted = false
            }
            "finalized" => external.status = ExternalSendStatus::Finalized,
            "cancelled" => external.status = ExternalSendStatus::Cancelled,
            "conflicted" => external.status = ExternalSendStatus::Conflicted,
            _ => unreachable!(),
        }
        assert!(w.build_rgb_fee_recovery(&request).is_err(), "{mode}");
    }
}

#[test]
fn original_must_be_expired_and_keep_its_immutable_request_and_complete_signature() {
    for mode in ["live", "malformed", "wrong_operation", "missing_witness"] {
        let (mut w, request, _, original) = fixture();
        let artifact = w.transfer_artifacts.get_mut(&original).unwrap();
        if mode == "missing_witness" {
            let mut signed = Psbt::from_str(artifact.signed_psbt.as_ref().unwrap()).unwrap();
            signed.inputs[0].final_script_witness = None;
            artifact.signed_psbt = Some(signed.to_string());
        } else {
            let binding = artifact
                .external_send
                .as_mut()
                .unwrap()
                .wallet_transfer
                .as_mut()
                .unwrap();
            if mode == "malformed" {
                binding.request_json = "{}".into();
            } else {
                let mut old: WalletRgbSendRequest =
                    serde_json::from_str(&binding.request_json).unwrap();
                if mode == "live" {
                    old.expires_at = now_seconds().unwrap() + 600;
                } else {
                    old.operation_id = "00000000-0000-4000-8000-000000000000".into();
                }
                binding.request_json = serde_json::to_string(&old).unwrap();
            }
        }
        assert!(w.build_rgb_fee_recovery(&request).is_err(), "{mode}");
    }
}

#[test]
fn wrong_network_stale_new_approval_fees_overflow_and_other_caller_holds_refuse() {
    let (mut w, request, _, _) = fixture();
    for fee in ["0", "1001", "01000", "-1", "18446744073709551616"] {
        let mut bad = request.clone();
        bad.fee_sats = fee.into();
        assert!(w.build_rgb_fee_recovery(&bad).is_err());
    }
    for expiry in ["1".into(), (now_seconds().unwrap() + 121).to_string()] {
        let mut bad = request.clone();
        bad.expires_at = expiry;
        assert!(w.build_rgb_fee_recovery(&bad).is_err());
    }
    let mut bad = request.clone();
    bad.excluded_inputs = vec![request.outpoint.clone()];
    assert!(w.build_rgb_fee_recovery(&bad).is_err());
    let mut bad = request.clone();
    bad.excluded_inputs = vec![OutPoint::null().to_string(); 2];
    assert!(w.build_rgb_fee_recovery(&bad).is_err());
    w.wallet_data.bitcoin_network = BitcoinNetwork::Mainnet;
    assert!(w.build_rgb_fee_recovery(&request).is_err());
}

#[test]
fn altered_destination_amount_input_value_and_nonbinding_sighashes_are_not_signed() {
    let (mut w, request, asset, _) = fixture();
    let prepared = w.build_rgb_fee_recovery(&request).unwrap();
    let original = Psbt::from_str(&prepared.psbt).unwrap();
    let foreign = wallet()
        ._get_new_address(KeychainKind::Internal)
        .unwrap()
        .script_pubkey();
    for mode in [
        "foreign",
        "colored_return",
        "amount",
        "input",
        "value",
        "none_sighash",
        "anyonecanpay",
        "version",
        "future_locktime",
        "relative_sequence",
    ] {
        let mut changed = original.clone();
        match mode {
            "foreign" => changed.unsigned_tx.output[0].script_pubkey = foreign.clone(),
            "colored_return" => {
                changed.unsigned_tx.output[0].script_pubkey = w
                    ._get_new_address(KeychainKind::External)
                    .unwrap()
                    .script_pubkey()
            }
            "amount" => changed.unsigned_tx.output[0].value = BdkAmount::from_sat(48_999),
            "input" => changed.unsigned_tx.input[0].previous_output = asset,
            "value" => {
                changed.inputs[0].witness_utxo.as_mut().unwrap().value = BdkAmount::from_sat(50_001)
            }
            "none_sighash" => {
                changed.inputs[0].sighash_type = Some(bitcoin::psbt::PsbtSighashType::from_u32(2))
            }
            "anyonecanpay" => {
                changed.inputs[0].sighash_type =
                    Some(bitcoin::psbt::PsbtSighashType::from_u32(0x81))
            }
            "version" => changed.unsigned_tx.version = bitcoin::transaction::Version::ONE,
            "future_locktime" => {
                changed.unsigned_tx.lock_time =
                    bitcoin::absolute::LockTime::from_height(201).unwrap()
            }
            "relative_sequence" => changed.unsigned_tx.input[0].sequence = bitcoin::Sequence(1),
            _ => unreachable!(),
        }
        assert!(
            w.checked_rgb_fee_recovery_psbt(&request, &changed.to_string(), false)
                .is_err(),
            "{mode}"
        );
    }
}

#[test]
fn new_stock_assignment_or_another_native_original_blocks_previously_reviewed_fee_input() {
    for mode in ["stock", "other_original"] {
        let (mut w, request, _, _) = fixture();
        let prepared = w.build_rgb_fee_recovery(&request).unwrap();
        if mode == "stock" {
            assign_stock(&w, request.outpoint.parse().unwrap(), false);
        } else {
            let mut other = Psbt::from_str(&prepared.psbt).unwrap();
            other.unsigned_tx.output[0].script_pubkey = w
                ._get_new_address(KeychainKind::Internal)
                .unwrap()
                .script_pubkey();
            retain_external(&mut w, &other, false);
        }
        assert!(
            w.checked_rgb_fee_recovery_psbt(&request, &prepared.psbt, false)
                .is_err(),
            "{mode}"
        );
    }
}
