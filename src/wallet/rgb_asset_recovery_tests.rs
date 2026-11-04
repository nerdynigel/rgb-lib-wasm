//! Real BDK key-path signatures, standard NIA genesis Stock and native DB on an
//! invented two-input graph. No live chain, valid outgoing consignment, network
//! request, delivery or broadcast: these are bounded authorization regressions.
use super::*;
use crate::wallet::online::safe_bitcoin::tests::{coin, retain_external, wallet};

fn graph(
    w: &Wallet,
    asset: &str,
    incoming: bool,
    status: TransferStatus,
    point: Option<OutPoint>,
    amount: u64,
    recipient: Option<String>,
) -> (i32, i32) {
    let batch = w
        .database
        .set_batch_transfer(DbBatchTransferActMod {
            status: ActiveValue::Set(status),
            min_confirmations: ActiveValue::Set(1),
            created_at: ActiveValue::Set(1),
            ..Default::default()
        })
        .unwrap();
    let a = w
        .database
        .set_asset_transfer(DbAssetTransferActMod {
            batch_transfer_idx: ActiveValue::Set(batch),
            asset_id: ActiveValue::Set(Some(asset.into())),
            user_driven: ActiveValue::Set(true),
            ..Default::default()
        })
        .unwrap();
    w.database
        .set_transfer(DbTransferActMod {
            asset_transfer_idx: ActiveValue::Set(a),
            incoming: ActiveValue::Set(incoming),
            requested_assignment: ActiveValue::Set(Some(Assignment::Fungible(amount))),
            recipient_id: ActiveValue::Set(recipient),
            ..Default::default()
        })
        .unwrap();
    if let Some(p) = point {
        let txo = w
            .database
            .get_txo(&Outpoint {
                txid: p.txid.to_string(),
                vout: p.vout,
            })
            .unwrap()
            .unwrap();
        w.database
            .set_coloring(DbColoringActMod {
                txo_idx: ActiveValue::Set(txo.idx),
                asset_transfer_idx: ActiveValue::Set(a),
                r#type: ActiveValue::Set(ColoringType::Issue),
                assignment: ActiveValue::Set(Assignment::Fungible(amount)),
                ..Default::default()
            })
            .unwrap();
    }
    (batch, a)
}

fn fixture() -> (Wallet, WalletRgbSendRequest, String, Vec<OutPoint>) {
    fixture_with_fee(false)
}

fn fixture_with_fee(with_fee: bool) -> (Wallet, WalletRgbSendRequest, String, Vec<OutPoint>) {
    let mut w = wallet();
    let p = vec![
        coin(&mut w, KeychainKind::External, 1000, true, false),
        coin(&mut w, KeychainKind::External, 1000, true, false),
    ];
    let contract = ContractBuilder::with(
        Identity::default(),
        NonInflatableAsset::schema(),
        NonInflatableAsset::types(),
        NonInflatableAsset::scripts(),
        w.chain_net(),
    )
    .add_global_state(
        "spec",
        AssetSpec {
            ticker: Ticker::try_from("FAKE".to_owned()).unwrap(),
            name: Name::try_from("Clearly fake recovery unit asset".to_owned()).unwrap(),
            details: None,
            precision: Precision::try_from(0u8).unwrap(),
        },
    )
    .unwrap()
    .add_global_state(
        "terms",
        ContractTerms {
            text: RicardianContract::default(),
            media: None,
        },
    )
    .unwrap()
    .add_global_state("issuedSupply", Amount::from(2u64))
    .unwrap()
    .add_fungible_state("assetOwner", w.get_builder_seal(p[0]), 1u64)
    .unwrap()
    .add_fungible_state("assetOwner", w.get_builder_seal(p[1]), 1u64)
    .unwrap()
    .issue_contract()
    .unwrap();
    let contract_id = contract.contract_id();
    let asset = contract_id.to_string();
    w.rgb_runtime()
        .unwrap()
        .import_contract(contract, &DumbResolver)
        .unwrap();
    for point in &p {
        graph(
            &w,
            &asset,
            true,
            TransferStatus::Settled,
            Some(*point),
            1,
            None,
        );
    }
    let destination = w._get_new_address(KeychainKind::External).unwrap();
    let recipient = crate::utils::recipient_id_from_script_buf(
        destination.script_pubkey(),
        BitcoinNetwork::Regtest,
    );
    graph(
        &w,
        &asset,
        true,
        TransferStatus::WaitingCounterparty,
        None,
        2,
        Some(recipient.clone()),
    );
    w.database
        .set_pending_witness_script(DbPendingWitnessScriptActMod {
            script: ActiveValue::Set(destination.script_pubkey().to_hex_string()),
            ..Default::default()
        })
        .unwrap();
    let original_destination = w._get_new_address(KeychainKind::External).unwrap();
    let mut original_inputs = p.clone();
    if with_fee {
        original_inputs.push(coin(&mut w, KeychainKind::Internal, 100_000, true, false));
    }
    let mut builder = w.bdk_wallet.build_tx();
    builder
        .add_utxos(&original_inputs)
        .unwrap()
        .manually_selected_only()
        .add_recipient(
            original_destination.script_pubkey(),
            BdkAmount::from_sat(1000),
        )
        .add_data(&bitcoin::script::PushBytesBuf::try_from(vec![4u8; 32]).unwrap())
        .fee_absolute(BdkAmount::from_sat(1000));
    let psbt = builder.finish().unwrap();
    let (original, batch) = retain_external(&mut w, &psbt, true);
    let mut old = WalletRgbSendRequest {
        operation_id: "01a19805-0462-759b-9050-b9b0feb9da8f".into(),
        asset_id: asset.clone(),
        recipient_id: crate::utils::recipient_id_from_script_buf(
            original_destination.script_pubkey(),
            BitcoinNetwork::Regtest,
        ),
        amount: 2,
        witness_sats: 1000,
        blinding: None,
        transport_endpoints: vec!["rpc://127.0.0.1:3000/json-rpc".into()],
        fee_rate: 1,
        min_confirmations: 1,
        excluded_inputs: vec![],
        expires_at: 1,
        recovery_of: None,
    };
    let old_binding = validate_request(&mut old).unwrap().binding;
    let mut signed = psbt;
    w._sign_psbt(&mut signed, None).unwrap();
    let signed_digest = <sha256::Hash as bitcoin::hashes::Hash>::hash(
        &bitcoin::consensus::serialize(&signed.clone().extract_tx().unwrap()),
    )
    .to_string();
    let mut txo_map = HashMap::new();
    let mut assignments_spent = HashMap::new();
    let old_asset = w
        .database
        .get_db_data(false)
        .unwrap()
        .asset_transfers
        .into_iter()
        .find(|a| a.batch_transfer_idx == batch)
        .unwrap()
        .idx;
    for point in &p {
        let dbp = Outpoint {
            txid: point.txid.to_string(),
            vout: point.vout,
        };
        let idx = w.database.get_txo(&dbp).unwrap().unwrap().idx;
        txo_map.insert(idx, dbp);
        assignments_spent.insert(idx, vec![Assignment::Fungible(1)]);
        w.database
            .set_coloring(DbColoringActMod {
                txo_idx: ActiveValue::Set(idx),
                asset_transfer_idx: ActiveValue::Set(old_asset),
                r#type: ActiveValue::Set(ColoringType::Input),
                assignment: ActiveValue::Set(Assignment::Fungible(1)),
                ..Default::default()
            })
            .unwrap();
    }
    let art = w.transfer_artifacts.get_mut(&original).unwrap();
    art.signed_psbt = Some(signed.to_string());
    art.external_send.as_mut().unwrap().wallet_transfer = Some(WalletTransferBinding {
        signing_attempted: true,
        broadcast_attempted: true,
        ..old_binding
    });
    art.asset_infos.insert(
        asset.clone(),
        InfoAssetTransfer {
            asset_info: AssetInfo {
                contract_id,
                reject_list_url: None,
            },
            recipients: vec![],
            asset_spend: AssetSpend {
                txo_map,
                assignments_collected: AssignmentsCollection {
                    fungible: 2,
                    ..Default::default()
                },
                input_btc_amt: 2000,
            },
            change: AssignmentsCollection::default(),
            original_assignments_needed: AssignmentsCollection {
                fungible: 2,
                ..Default::default()
            },
            assignments_needed: AssignmentsCollection {
                fungible: 2,
                ..Default::default()
            },
            assignments_spent,
            main_transition: TypeOfTransition::Transfer,
        },
    );
    let request = WalletRgbSendRequest {
        operation_id: "01a19805-0462-759b-9050-b9b0feb9da8a".into(),
        asset_id: asset,
        recipient_id: recipient,
        amount: 2,
        witness_sats: 1000,
        blinding: None,
        transport_endpoints: old.transport_endpoints,
        fee_rate: 1,
        min_confirmations: 1,
        excluded_inputs: vec![],
        expires_at: current_seconds().unwrap() + 60,
        recovery_of: Some(WalletRgbRecoveryBinding {
            version: 1,
            original_operation_id: old.operation_id,
            original_txid: original.clone(),
            original_signed_transaction_sha256: signed_digest,
            max_fee_sats: 1000,
        }),
    };
    (w, request, original, original_inputs)
}

#[test]
fn original_bound_fee_selection_preserves_every_hold_and_refuses_unrelated_spending() {
    let (w, r, original, points) = fixture_with_fee(true);
    let before = serde_json::to_vec(w.database.as_ref()).unwrap();
    let artifacts = serde_json::to_vec(&w.transfer_artifacts).unwrap();
    assert_eq!(w.bitcoin_spend_view().unwrap().total_sats, "0");
    let view = w.bitcoin_rgb_recovery_fee_view(&r).unwrap();
    assert_eq!(view.total_sats, "100000");
    assert_eq!(view.inputs.len(), 1);
    assert_eq!(view.inputs[0].outpoint, points[2].to_string());
    assert_eq!(w.bitcoin_spend_view().unwrap().total_sats, "0");
    assert!(
        w.managed_rgb_input_holds(None)
            .unwrap()
            .contains(&points[2])
    );
    assert_eq!(serde_json::to_vec(w.database.as_ref()).unwrap(), before);
    assert_eq!(
        serde_json::to_vec(&w.transfer_artifacts).unwrap(),
        artifacts
    );
    let mut changed = r;
    changed
        .recovery_of
        .as_mut()
        .unwrap()
        .original_signed_transaction_sha256 = "ee".repeat(32);
    assert!(w.bitcoin_rgb_recovery_fee_view(&changed).is_err());
    assert_eq!(
        w.bitcoin_rgb_fee_inputs(
            &serde_json::from_str::<WalletRgbSendRequest>(
                &w.transfer_artifacts[&original]
                    .external_send
                    .as_ref()
                    .unwrap()
                    .wallet_transfer
                    .as_ref()
                    .unwrap()
                    .request_json
            )
            .unwrap()
            .operation_id
        )
        .unwrap()
        .len(),
        1
    );
}

#[test]
fn source_and_child_keep_exact_fee_evidence_but_never_exempt_a_third_hold() {
    let (mut w, mut r, original, points) = fixture_with_fee(true);
    let recipient = XChainNet::<Beneficiary>::from_str(&r.recipient_id)
        .unwrap()
        .into_inner();
    let Beneficiary::WitnessVout(pay_to, None) = recipient else {
        panic!("own witness required")
    };
    let mut builder = w.bdk_wallet.build_tx();
    builder
        .add_utxos(&points)
        .unwrap()
        .manually_selected_only()
        .add_recipient(pay_to.to_script(), BdkAmount::from_sat(1000))
        .add_data(&bitcoin::script::PushBytesBuf::try_from(vec![9u8; 32]).unwrap())
        .fee_absolute(BdkAmount::from_sat(1000));
    let psbt = builder.finish().unwrap();
    w.check_wallet_rgb_recovery_transaction(&r, &psbt).unwrap();
    let (child, _) = retain_external(&mut w, &psbt, true);
    w.transfer_artifacts
        .get_mut(&child)
        .unwrap()
        .external_send
        .as_mut()
        .unwrap()
        .wallet_transfer = Some(validate_request(&mut r).unwrap().binding);
    assert_eq!(
        w.wallet_rgb_recovery_peer_for_tx(&original).unwrap(),
        Some(child.clone())
    );
    assert_eq!(
        w.wallet_rgb_recovery_peer_for_tx(&child).unwrap(),
        Some(original.clone())
    );
    let old_operation = r
        .recovery_of
        .as_ref()
        .unwrap()
        .original_operation_id
        .clone();
    assert_eq!(
        w.bitcoin_rgb_fee_inputs(&old_operation).unwrap()[0].outpoint,
        points[2].to_string()
    );
    assert_eq!(
        w.bitcoin_rgb_fee_inputs(&r.operation_id).unwrap()[0].outpoint,
        points[2].to_string()
    );
    assert_eq!(w.bitcoin_spend_view().unwrap().total_sats, "0");
    w.check_wallet_rgb_recovery_transaction(&r, &psbt).unwrap();
    let mut builder = w.bdk_wallet.build_tx();
    builder
        .add_utxo(points[2])
        .unwrap()
        .manually_selected_only()
        .add_recipient(pay_to.to_script(), BdkAmount::from_sat(1000))
        .fee_absolute(BdkAmount::from_sat(1000));
    let third = builder.finish().unwrap();
    retain_external(&mut w, &third, false);
    assert!(w.bitcoin_rgb_fee_inputs(&old_operation).unwrap().is_empty());
    assert!(
        w.bitcoin_rgb_fee_inputs(&r.operation_id)
            .unwrap()
            .is_empty()
    );
    assert!(w.check_wallet_rgb_recovery_transaction(&r, &psbt).is_err());
    assert_eq!(w.bitcoin_spend_view().unwrap().total_sats, "0");
}

#[test]
fn dispatched_two_rgb_inputs_qualify_without_any_plain_bitcoin_fee_input_and_keep_all_state() {
    let (w, r, id, points) = fixture();
    let before = serde_json::to_vec(w.database.as_ref()).unwrap();
    let old = w
        .wallet_rgb_send_data(
            r.recovery_of
                .as_ref()
                .unwrap()
                .original_operation_id
                .clone(),
        )
        .unwrap();
    assert!(
        w.bitcoin_rgb_fee_inputs(&r.recovery_of.as_ref().unwrap().original_operation_id)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        w.checked_wallet_rgb_recovery_source(&r).unwrap(),
        Some(id.clone())
    );
    let rows = w
        .wallet_rgb_recovery_candidate_inputs(&id, &r, &mut HashSet::new())
        .unwrap();
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().all(|u| u.rgb_allocations.len() == 1
        && u.rgb_allocations[0].assignment == Assignment::Fungible(1)));
    assert_eq!(w.bitcoin_spend_view().unwrap().total_sats, "0");
    let held = w.managed_rgb_input_holds(None).unwrap();
    assert!(points.iter().all(|p| held.contains(p)));
    assert_eq!(serde_json::to_vec(w.database.as_ref()).unwrap(), before);
    assert_eq!(
        w.wallet_rgb_send_data(r.recovery_of.unwrap().original_operation_id)
            .unwrap(),
        old
    );
}

#[test]
fn changed_original_identity_signature_network_state_or_asset_never_qualifies() {
    for mode in [
        "digest",
        "txid",
        "operation",
        "asset",
        "restored",
        "live",
        "unsigned",
        "not_attempted",
        "finalized",
        "mainnet",
        "foreign_state",
    ] {
        let (mut w, mut r, id, p) = fixture();
        match mode {
            "digest" => {
                r.recovery_of
                    .as_mut()
                    .unwrap()
                    .original_signed_transaction_sha256 = "0".repeat(64)
            }
            "txid" => r.recovery_of.as_mut().unwrap().original_txid = "0".repeat(64),
            "operation" => {
                r.recovery_of.as_mut().unwrap().original_operation_id = r.operation_id.clone()
            }
            "asset" => r.asset_id = "rgb:Kpai9_2X-uhBiC5K-moZtGkh-hwThNSE-h96Pbck-gfJQRHY".into(),
            "mainnet" => w.wallet_data.bitcoin_network = BitcoinNetwork::Mainnet,
            "foreign_state" => {
                crate::wallet::online::safe_bitcoin::tests::assign_stock(&w, p[0], true)
            }
            "unsigned" => w.transfer_artifacts.get_mut(&id).unwrap().signed_psbt = None,
            _ => {
                let e = w
                    .transfer_artifacts
                    .get_mut(&id)
                    .unwrap()
                    .external_send
                    .as_mut()
                    .unwrap();
                if mode == "finalized" {
                    e.status = ExternalSendStatus::Finalized;
                } else {
                    let b = e.wallet_transfer.as_mut().unwrap();
                    if mode == "restored" {
                        b.restored = true;
                    }
                    if mode == "not_attempted" {
                        b.signing_attempted = false;
                    }
                    if mode == "live" {
                        let mut old: WalletRgbSendRequest =
                            serde_json::from_str(&b.request_json).unwrap();
                        old.expires_at = current_seconds().unwrap() + 600;
                        b.request_json = serde_json::to_string(&old).unwrap();
                    }
                }
            }
        }
        assert!(w.checked_wallet_rgb_recovery_source(&r).is_err(), "{mode}");
    }
}

#[test]
fn all_rgb_must_return_to_an_exact_wallet_owned_receive_record() {
    let (mut w, r, _, _) = fixture();
    for amount in [1, 3, u64::MAX] {
        let mut bad = r.clone();
        bad.amount = amount;
        assert!(w.checked_wallet_rgb_recovery_source(&bad).is_err());
    }
    let mut bad = r.clone();
    bad.recipient_id = crate::utils::recipient_id_from_script_buf(
        wallet()
            ._get_new_address(KeychainKind::External)
            .unwrap()
            .script_pubkey(),
        BitcoinNetwork::Regtest,
    );
    assert!(w.checked_wallet_rgb_recovery_source(&bad).is_err());
    let mut bad = r.clone();
    bad.recipient_id = crate::utils::recipient_id_from_script_buf(
        w._get_new_address(KeychainKind::Internal)
            .unwrap()
            .script_pubkey(),
        BitcoinNetwork::Regtest,
    );
    assert!(w.checked_wallet_rgb_recovery_source(&bad).is_err());
    let mut bad = r;
    bad.recipient_id = crate::utils::recipient_id_from_script_buf(
        w._get_new_address(KeychainKind::External)
            .unwrap()
            .script_pubkey(),
        BitcoinNetwork::Regtest,
    );
    assert!(
        w.checked_wallet_rgb_recovery_source(&bad).is_err(),
        "an owned address without its incoming RGB record is insufficient"
    );
}

#[test]
fn third_native_holds_and_caller_exclusions_remain_effective() {
    let (mut w, r, id, p) = fixture();
    let mut excluded = HashSet::from([p[0]]);
    assert!(
        w.wallet_rgb_recovery_candidate_inputs(&id, &r, &mut excluded)
            .is_err()
    );
    let mut other = Psbt::from_str(
        &w.transfer_artifacts[&id]
            .external_send
            .as_ref()
            .unwrap()
            .prepared_psbt,
    )
    .unwrap();
    other.unsigned_tx.output[0].value = BdkAmount::from_sat(999);
    retain_external(&mut w, &other, false);
    assert!(
        w.wallet_rgb_recovery_candidate_inputs(&id, &r, &mut HashSet::new())
            .is_err()
    );
}

#[test]
fn own_new_transaction_binds_fee_all_original_rgb_and_every_destination() {
    let (mut w, r, _, p) = fixture();
    let script = match XChainNet::<Beneficiary>::from_str(&r.recipient_id)
        .unwrap()
        .into_inner()
    {
        Beneficiary::WitnessVout(v, _) => v.to_script(),
        _ => unreachable!(),
    };
    let mut b = w.bdk_wallet.build_tx();
    b.add_utxos(&p)
        .unwrap()
        .manually_selected_only()
        .add_recipient(script, BdkAmount::from_sat(1000))
        .fee_absolute(BdkAmount::from_sat(1000));
    let psbt = b.finish().unwrap();
    w.check_wallet_rgb_recovery_transaction(&r, &psbt).unwrap();
    let mut bad = r.clone();
    bad.recovery_of.as_mut().unwrap().max_fee_sats = 999;
    assert!(
        w.check_wallet_rgb_recovery_transaction(&bad, &psbt)
            .is_err()
    );
    let mut bad = psbt.clone();
    bad.unsigned_tx.input.remove(0);
    bad.inputs.remove(0);
    assert!(w.check_wallet_rgb_recovery_transaction(&r, &bad).is_err());
    let mut bad = psbt;
    bad.unsigned_tx.output[0].script_pubkey = wallet()
        ._get_new_address(KeychainKind::External)
        .unwrap()
        .script_pubkey();
    assert!(w.check_wallet_rgb_recovery_transaction(&r, &bad).is_err());
}

#[test]
fn explicit_marker_is_canonical_and_legacy_ordinary_bytes_are_unchanged() {
    let (_, mut r, _, _) = fixture();
    let serialized = validate_request(&mut r).unwrap().binding.request_json;
    assert!(serialized.contains("recovery_of"));
    let mut decoded: WalletRgbSendRequest = serde_json::from_str(&serialized).unwrap();
    assert_eq!(
        validate_request(&mut decoded).unwrap().binding.request_json,
        serialized
    );
    decoded.recovery_of = None;
    assert!(
        !validate_request(&mut decoded)
            .unwrap()
            .binding
            .request_json
            .contains("recovery_of")
    );
    for fee in [0, 10001, u64::MAX] {
        let mut bad = r.clone();
        bad.recovery_of.as_mut().unwrap().max_fee_sats = fee;
        assert!(validate_request(&mut bad).is_err());
    }
}

#[test]
fn one_original_cannot_authorize_competing_recoveries_and_ordinary_entry_refuses_marker() {
    let (mut w, r, id, p) = fixture();
    let receiver = match XChainNet::<Beneficiary>::from_str(&r.recipient_id)
        .unwrap()
        .into_inner()
    {
        Beneficiary::WitnessVout(v, _) => v.to_script(),
        _ => unreachable!(),
    };
    let mut b = w.bdk_wallet.build_tx();
    b.add_utxos(&p)
        .unwrap()
        .manually_selected_only()
        .add_recipient(receiver, BdkAmount::from_sat(1000))
        .add_data(&bitcoin::script::PushBytesBuf::try_from(vec![7u8; 32]).unwrap())
        .fee_absolute(BdkAmount::from_sat(1000));
    let new = b.finish().unwrap();
    let (new_id, _) = retain_external(&mut w, &new, false);
    let mut canonical = r.clone();
    let binding = validate_request(&mut canonical).unwrap().binding;
    let old_info = w.transfer_artifacts[&id].asset_infos.clone();
    let a = w.transfer_artifacts.get_mut(&new_id).unwrap();
    a.asset_infos = old_info;
    a.external_send.as_mut().unwrap().wallet_transfer = Some(binding);
    assert_eq!(w.checked_wallet_rgb_recovery_source(&r).unwrap(), Some(id));
    w.wallet_rgb_signing_preflight(
        &r.operation_id,
        &new.to_string(),
        current_seconds().unwrap(),
    )
    .unwrap();
    let mut other = r.clone();
    other.operation_id = "01a19805-0462-759b-9050-b9b0feb9da8b".into();
    assert!(w.checked_wallet_rgb_recovery_source(&other).is_err());
    let before = serde_json::to_vec(&w.transfer_artifacts).unwrap();
    let rt = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    assert!(
        rt.block_on(w.prepare_wallet_rgb_send(
            Online {
                id: 1,
                indexer_url: "http://not-contacted.invalid".into()
            },
            r
        ))
        .is_err()
    );
    assert_eq!(serde_json::to_vec(&w.transfer_artifacts).unwrap(), before);
}

#[test]
fn raw_original_and_recovery_purpose_survive_native_artifact_serialization() {
    let (w, r, id, _) = fixture();
    let bytes = serde_json::to_vec(&w.transfer_artifacts).unwrap();
    let reopened: HashMap<String, TransferArtifacts> = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        reopened[&id].signed_psbt,
        w.transfer_artifacts[&id].signed_psbt
    );
    assert_eq!(
        reopened[&id]
            .external_send
            .as_ref()
            .unwrap()
            .wallet_transfer,
        w.transfer_artifacts[&id]
            .external_send
            .as_ref()
            .unwrap()
            .wallet_transfer
    );
    let restored: WalletRgbSendRequest =
        serde_json::from_str(&serde_json::to_string(&r).unwrap()).unwrap();
    assert_eq!(restored.recovery_of, r.recovery_of);
}
