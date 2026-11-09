//! Actual local BDK/database/Stock graphs; no network, funds or settlement.
use super::super::safe_bitcoin::tests::{assign_stock, coin, retain_external, wallet};
use super::*;

fn issue(wallet: &Wallet, points: &[(OutPoint, u64)], name: &str) -> String {
    let spec = AssetSpec {
        ticker: Ticker::try_from("FAKE".to_owned()).unwrap(),
        name: Name::try_from(name.to_owned()).unwrap(),
        details: None,
        precision: Precision::try_from(0u8).unwrap(),
    };
    let mut builder = ContractBuilder::with(
        Identity::default(),
        NonInflatableAsset::schema(),
        NonInflatableAsset::types(),
        NonInflatableAsset::scripts(),
        wallet.chain_net(),
    )
    .add_global_state("spec", spec)
    .unwrap()
    .add_global_state(
        "terms",
        ContractTerms {
            text: RicardianContract::default(),
            media: None,
        },
    )
    .unwrap()
    .add_global_state(
        "issuedSupply",
        Amount::from(points.iter().map(|p| p.1).sum::<u64>()),
    )
    .unwrap();
    for (point, units) in points {
        builder = builder
            .add_fungible_state("assetOwner", wallet.get_builder_seal(*point), *units)
            .unwrap();
    }
    let contract = builder.issue_contract().unwrap();
    let id = contract.contract_id().to_string();
    wallet
        .rgb_runtime()
        .unwrap()
        .import_contract(contract, &DumbResolver)
        .unwrap();
    id
}

fn allocation(wallet: &Wallet, point: OutPoint, id: &str, amount: u64) {
    let batch = wallet
        .database
        .set_batch_transfer(DbBatchTransferActMod {
            status: ActiveValue::Set(TransferStatus::Settled),
            min_confirmations: ActiveValue::Set(1),
            ..Default::default()
        })
        .unwrap();
    let asset = wallet
        .database
        .set_asset_transfer(DbAssetTransferActMod {
            batch_transfer_idx: ActiveValue::Set(batch),
            asset_id: ActiveValue::Set(Some(id.into())),
            ..Default::default()
        })
        .unwrap();
    wallet
        .database
        .set_transfer(DbTransferActMod {
            asset_transfer_idx: ActiveValue::Set(asset),
            incoming: ActiveValue::Set(true),
            ..Default::default()
        })
        .unwrap();
    let txo = wallet
        .database
        .get_txo(&Outpoint {
            txid: point.txid.to_string(),
            vout: point.vout,
        })
        .unwrap()
        .unwrap()
        .idx;
    wallet
        .database
        .set_coloring(DbColoringActMod {
            txo_idx: ActiveValue::Set(txo),
            asset_transfer_idx: ActiveValue::Set(asset),
            r#type: ActiveValue::Set(ColoringType::Issue),
            assignment: ActiveValue::Set(Assignment::Fungible(amount)),
            ..Default::default()
        })
        .unwrap();
}

fn fixture(confirmed: bool, coinbase: bool) -> (Wallet, OutPoint, String) {
    let mut w = wallet();
    let point = coin(&mut w, KeychainKind::External, 100000, confirmed, coinbase);
    let id = issue(&w, &[(point, 3)], "Fake input qualification");
    allocation(&w, point, &id, 3);
    (w, point, id)
}

#[test]
fn actual_fragmented_inputs_have_exact_values_scripts_and_no_state_mutation() {
    let mut w = wallet();
    let a = coin(&mut w, KeychainKind::External, 1000, true, false);
    let b = coin(&mut w, KeychainKind::External, 2000, true, false);
    let id = issue(&w, &[(a, 3), (b, 7)], "Fake fragmented qualification");
    allocation(&w, a, &id, 3);
    allocation(&w, b, &id, 7);
    let before_db = serde_json::to_vec(w.database.as_ref()).unwrap();
    let before_stock =
        crate::wallet::idb_store::serialize_stock(w.rgb_stock.borrow().as_ref().unwrap()).unwrap();
    let before_artifacts = serde_json::to_vec(&w.transfer_artifacts).unwrap();
    let sequence = w.idb_sequence.load(std::sync::atomic::Ordering::SeqCst);
    let view = w.asset_spend_view(id.clone()).unwrap();
    assert_eq!(view.contract_id, id);
    assert_eq!(view.total_units, "10");
    assert_eq!(view.total_sats, "3000");
    assert_eq!(view.inputs.len(), 2);
    assert!(view.inputs[0].outpoint < view.inputs[1].outpoint);
    for row in &view.inputs {
        let point = OutPoint::from_str(&row.outpoint).unwrap();
        let actual = w.bdk_wallet.get_utxo(point).unwrap();
        assert_eq!(row.input_sats, actual.txout.value.to_sat().to_string());
        assert_eq!(row.script_hex, actual.txout.script_pubkey.to_hex_string());
        assert_eq!(row.rgb_units, if point == a { "3" } else { "7" });
    }
    assert_eq!(serde_json::to_vec(w.database.as_ref()).unwrap(), before_db);
    assert_eq!(
        crate::wallet::idb_store::serialize_stock(w.rgb_stock.borrow().as_ref().unwrap()).unwrap(),
        before_stock
    );
    assert_eq!(
        serde_json::to_vec(&w.transfer_artifacts).unwrap(),
        before_artifacts
    );
    assert_eq!(
        w.idb_sequence.load(std::sync::atomic::Ordering::SeqCst),
        sequence
    );
}

#[test]
fn unconfirmed_or_immature_carriers_are_never_available() {
    for (confirmed, coinbase) in [(false, false), (true, true)] {
        let (w, _, id) = fixture(confirmed, coinbase);
        let view = w.asset_spend_view(id).unwrap();
        assert!(view.inputs.is_empty());
        assert_eq!(view.total_units, "0");
        assert_eq!(view.total_sats, "0");
    }
}

#[test]
fn inventory_proof_exports_standard_selected_state_without_any_wallet_mutation() {
    let mut w = wallet();
    let a = coin(&mut w, KeychainKind::External, 1000, true, false);
    let b = coin(&mut w, KeychainKind::External, 2000, true, false);
    let id = issue(&w, &[(a, 3), (b, 7)], "Fake inventory proof");
    allocation(&w, a, &id, 3);
    allocation(&w, b, &id, 7);
    let before_db = serde_json::to_vec(w.database.as_ref()).unwrap();
    let before_stock =
        crate::wallet::idb_store::serialize_stock(w.rgb_stock.borrow().as_ref().unwrap()).unwrap();
    let before_artifacts = serde_json::to_vec(&w.transfer_artifacts).unwrap();
    let sequence = w.idb_sequence.load(std::sync::atomic::Ordering::SeqCst);
    let before_bdk = serde_json::to_vec(&w.bdk_wallet.staged()).unwrap();
    let proof = w
        .asset_inventory_proof(id.clone(), vec![b.to_string()])
        .unwrap();
    assert_eq!(proof.version, 1);
    assert_eq!(
        proof.scope,
        "standard-rgb-inventory-proof-not-ownership-or-reservation"
    );
    assert_eq!(proof.selected.total_sats, "2000");
    assert_eq!(proof.selected.total_units, "7");
    assert_eq!(proof.selected.inputs.len(), 1);
    assert_eq!(proof.selected.inputs[0].outpoint, b.to_string());
    let bytes = hex::decode(&proof.consignment_hex).unwrap();
    let transfer = RgbTransfer::load(bytes.as_slice()).unwrap();
    assert_eq!(transfer.contract_id().to_string(), id);
    assert_eq!(
        AssetSchema::try_from(transfer.schema_id()).unwrap(),
        AssetSchema::Nia
    );
    assert!(
        transfer
            .clone()
            .validate(&DumbResolver, &ValidationConfig::default())
            .is_err()
    );
    transfer
        .validate(
            &DumbResolver,
            &ValidationConfig {
                chain_net: w.chain_net(),
                trusted_typesystem: AssetSchema::Nia.types(),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(serde_json::to_vec(w.database.as_ref()).unwrap(), before_db);
    assert_eq!(
        crate::wallet::idb_store::serialize_stock(w.rgb_stock.borrow().as_ref().unwrap()).unwrap(),
        before_stock
    );
    assert_eq!(
        serde_json::to_vec(&w.transfer_artifacts).unwrap(),
        before_artifacts
    );
    assert_eq!(
        serde_json::to_vec(&w.bdk_wallet.staged()).unwrap(),
        before_bdk
    );
    assert_eq!(
        w.idb_sequence.load(std::sync::atomic::Ordering::SeqCst),
        sequence
    );
    assert_eq!(
        w.asset_inventory_proof(id, vec![b.to_string()]).unwrap(),
        proof
    );
}

#[test]
fn inventory_proof_refuses_noncanonical_duplicate_foreign_or_unqualified_selections() {
    let (mut w, point, id) = fixture(true, false);
    let other = OutPoint {
        txid: bitcoin::Txid::from_str(&"ff".repeat(32)).unwrap(),
        vout: 0,
    };
    let mut unsorted = vec![point.to_string(), other.to_string()];
    unsorted.sort();
    unsorted.reverse();
    for requested in [
        vec![],
        vec![point.to_string(); 2],
        vec![point.to_string(); 33],
        vec![other.to_string()],
        vec!["invalid".into()],
        vec![format!("{}:00", point.txid)],
        vec![point.to_string().to_uppercase()],
        unsorted,
    ] {
        assert!(w.asset_inventory_proof(id.clone(), requested).is_err());
    }
    for asset in ["USDT".to_string(), "rgb:invalid".to_string()] {
        assert!(
            w.asset_inventory_proof(asset, vec![point.to_string()])
                .is_err()
        );
    }
    w.wallet_data.bitcoin_network = BitcoinNetwork::Mainnet;
    assert!(
        w.asset_inventory_proof(id, vec![point.to_string()])
            .is_err()
    );
    for (confirmed, coinbase) in [(false, false), (true, true)] {
        let (w, point, id) = fixture(confirmed, coinbase);
        assert!(
            w.asset_inventory_proof(id, vec![point.to_string()])
                .is_err()
        );
    }
}

#[test]
fn ticker_malformed_unknown_contract_and_non_regtest_are_refused() {
    let (mut w, _, id) = fixture(true, false);
    for bad in ["USDT", "", "rgb:invalid", &"x".repeat(193)] {
        assert!(w.asset_spend_view(bad.into()).is_err());
    }
    let mut other = wallet();
    let p = coin(&mut other, KeychainKind::External, 1000, true, false);
    let unknown = issue(&other, &[(p, 1)], "Unknown fake contract");
    assert!(w.asset_spend_view(unknown).is_err());
    w.wallet_data.bitcoin_network = BitcoinNetwork::Mainnet;
    assert!(matches!(
        w.asset_spend_view(id),
        Err(Error::InvalidRecipientNetwork)
    ));
}

#[test]
fn positive_display_without_exact_stock_and_overflow_fail_closed() {
    let (w, p, id) = fixture(true, false);
    allocation(&w, p, &id, 1);
    assert!(w.asset_spend_view(id.clone()).is_err());
    allocation(&w, p, &id, u64::MAX);
    assert!(w.asset_spend_view(id).is_err());
}

#[test]
fn hidden_stock_without_database_is_not_invented_as_spendable() {
    let mut w = wallet();
    let p = coin(&mut w, KeychainKind::External, 1000, true, false);
    let id = issue(&w, &[(p, 3)], "Fake hidden stock");
    assert!(w.asset_spend_view(id).unwrap().inputs.is_empty());
    assert!(w.bitcoin_spend_view().unwrap().inputs.is_empty());
}

#[test]
fn other_contracts_and_hidden_inflation_rights_never_burn_with_selected_asset() {
    let (w, p, id) = fixture(true, false);
    let other = issue(&w, &[(p, 2)], "Other fake contract on same seal");
    assert_ne!(id, other);
    assert!(w.asset_spend_view(id.clone()).unwrap().inputs.is_empty());
    assert!(w.asset_inventory_proof(id, vec![p.to_string()]).is_err());
    let (w, p, id) = fixture(true, false);
    assign_stock(&w, p, true);
    assert!(w.asset_spend_view(id).unwrap().inputs.is_empty());
}

#[test]
fn native_original_hold_excludes_the_entire_carrier_without_releasing_history() {
    let (mut w, p, id) = fixture(true, false);
    let address = w
        ._get_new_address(KeychainKind::Internal)
        .unwrap()
        .script_pubkey();
    let change = w
        ._get_new_address(KeychainKind::Internal)
        .unwrap()
        .script_pubkey();
    let (psbt, _) = w
        .prepare_psbt_with_change(
            HashSet::from([p]),
            &vec![(address, 1000)],
            w._check_fee_rate(3).unwrap(),
            None,
            change,
        )
        .unwrap();
    let (txid, _) = retain_external(&mut w, &psbt, false);
    let before = serde_json::to_vec(&w.transfer_artifacts).unwrap();
    assert!(w.asset_spend_view(id.clone()).unwrap().inputs.is_empty());
    assert!(
        w.asset_inventory_proof(id.clone(), vec![p.to_string()])
            .is_err()
    );
    assert_eq!(serde_json::to_vec(&w.transfer_artifacts).unwrap(), before);
    w.transfer_artifacts.remove(&txid);
    assert!(w.asset_spend_view(id).is_err());
}

#[test]
fn large_native_asset_units_remain_exact_decimal_strings() {
    let mut w = wallet();
    let p = coin(&mut w, KeychainKind::External, 1000, true, false);
    let units = 9_007_199_254_740_993u64;
    let id = issue(&w, &[(p, units)], "Large fake unit qualification");
    allocation(&w, p, &id, units);
    let view = w.asset_spend_view(id).unwrap();
    assert_eq!(view.inputs[0].rgb_units, "9007199254740993");
    assert_eq!(view.total_units, "9007199254740993");
    let encoded = serde_json::to_value(view).unwrap();
    assert!(encoded["total_units"].is_string());
    assert_eq!(encoded["inputs"][0]["rgb_units"], "9007199254740993");
}

#[test]
fn pending_missing_or_spent_database_carriers_do_not_become_available() {
    for flag in ["pending_witness", "missing", "spent"] {
        let (w, p, id) = fixture(true, false);
        let mut txo: DbTxoActMod = w
            .database
            .get_txo(&Outpoint {
                txid: p.txid.to_string(),
                vout: p.vout,
            })
            .unwrap()
            .unwrap()
            .into();
        match flag {
            "pending_witness" => txo.pending_witness = ActiveValue::Set(true),
            "missing" => txo.exists = ActiveValue::Set(false),
            "spent" => txo.spent = ActiveValue::Set(true),
            _ => unreachable!(),
        }
        w.database.update_txo(txo).unwrap();
        assert!(w.asset_spend_view(id).unwrap().inputs.is_empty());
    }
}

#[test]
fn wrong_native_prevout_amount_is_not_accepted_from_database() {
    let (w, p, id) = fixture(true, false);
    let mut txo: DbTxoActMod = w
        .database
        .get_txo(&Outpoint {
            txid: p.txid.to_string(),
            vout: p.vout,
        })
        .unwrap()
        .unwrap()
        .into();
    txo.btc_amount = ActiveValue::Set("100001".into());
    w.database.update_txo(txo).unwrap();
    assert!(w.asset_spend_view(id).is_err());
}

// These are real BDK signatures on synthetic BIP-322 message transactions over
// invented local graphs. No network funding, spending or settlement claim.
fn control_challenge(
    w: &Wallet,
    points: &[OutPoint],
    rgb: Option<&str>,
) -> InventoryControlChallenge {
    let contract = rgb.unwrap_or("rgb:Kpai9_2X-uhBiC5K-moZtGkh-hwThNSE-h96Pbck-gfJQRHY");
    let mut points = points.iter().map(ToString::to_string).collect::<Vec<_>>();
    points.sort();
    let (height, hash, inputs, consignment) = if rgb.is_some() {
        let p = w.asset_inventory_proof(contract.into(), points).unwrap();
        (
            p.selected.tip_height,
            p.selected.tip_hash,
            p.selected.inputs,
            <sha256::Hash as bitcoin::hashes::Hash>::hash(&hex::decode(p.consignment_hex).unwrap())
                .to_string(),
        )
    } else {
        let v = w.bitcoin_spend_view().unwrap();
        (
            v.tip_height,
            v.tip_hash,
            points
                .iter()
                .map(|p| {
                    let i = v.inputs.iter().find(|i| &i.outpoint == p).unwrap();
                    AssetSpendInput {
                        outpoint: i.outpoint.clone(),
                        input_sats: i.amount_sats.clone(),
                        rgb_units: "0".into(),
                        script_hex: i.script_hex.clone(),
                    }
                })
                .collect(),
            "0".repeat(64),
        )
    };
    InventoryControlChallenge {
        version: 1,
        purpose: "inventory-control-only".into(),
        network: "regtest".into(),
        proof_kind: if rgb.is_some() { "rgb" } else { "bitcoin" }.into(),
        origin_sha256: "a".repeat(64),
        session_sha256: "b".repeat(64),
        owner_sha256: "c".repeat(64),
        nonce: "d".repeat(64),
        expires_at: (super::super::safe_bitcoin::unix_seconds().unwrap() + 60).to_string(),
        contract_id: contract.into(),
        consignment_sha256: consignment,
        tip_height: height,
        tip_hash: hash,
        inputs,
    }
}

fn control_snapshot(w: &Wallet) -> Vec<u8> {
    serde_json::to_vec(&(
        w.database.as_ref(),
        crate::wallet::idb_store::serialize_stock(w.rgb_stock.borrow().as_ref().unwrap()).unwrap(),
        &w.transfer_artifacts,
        w.bdk_wallet.staged(),
        w.idb_sequence.load(std::sync::atomic::Ordering::SeqCst),
    ))
    .unwrap()
}

fn verify_control(proof: &InventoryControlProof) {
    assert_eq!(proof.scheme, "bip322-simple-p2tr-default");
    assert_eq!(proof.proofs.len(), proof.challenge.inputs.len());
    for (p, i) in proof.proofs.iter().zip(&proof.challenge.inputs) {
        assert_eq!(p.outpoint, i.outpoint);
        assert_eq!(p.witness_hex.len(), 132);
        assert!(p.witness_hex.starts_with("0140"));
        let witness: bitcoin::Witness =
            bitcoin::consensus::deserialize(&hex::decode(&p.witness_hex).unwrap()).unwrap();
        let address = BdkAddress::from_script(
            &bitcoin::ScriptBuf::from_hex(&i.script_hex).unwrap(),
            BdkNetwork::Regtest,
        )
        .unwrap();
        bip322::verify_simple(&address, proof.challenge.message(i), witness).unwrap();
    }
}

#[test]
fn inventory_control_signs_both_real_bdk_keychains_without_any_durable_mutation() {
    let mut w = wallet();
    let a = coin(&mut w, KeychainKind::Internal, 10000, true, false);
    let b = coin(&mut w, KeychainKind::External, 20000, true, false);
    let c = control_challenge(&w, &[a, b], None);
    let before = control_snapshot(&w);
    let p = w.prove_inventory_control(c.clone()).unwrap();
    assert_eq!(p.challenge, c);
    verify_control(&p);
    assert_eq!(control_snapshot(&w), before);
    let value = serde_json::to_value(p).unwrap();
    assert_eq!(value.as_object().unwrap().len(), 4);
    assert!(value.get("psbt").is_none() && value.get("transaction").is_none());
}

#[test]
fn inventory_control_signs_exact_standard_rgb_inputs_without_stock_or_artifact_changes() {
    let mut w = wallet();
    let a = coin(&mut w, KeychainKind::External, 1000, true, false);
    let b = coin(&mut w, KeychainKind::External, 2000, true, false);
    let id = issue(&w, &[(a, 3), (b, 7)], "Fake control proof");
    allocation(&w, a, &id, 3);
    allocation(&w, b, &id, 7);
    let mut c = control_challenge(&w, &[a, b], Some(&id));
    let hash =
        |text: &str| <sha256::Hash as bitcoin::hashes::Hash>::hash(text.as_bytes()).to_string();
    c.origin_sha256 = hash("https://market.example");
    c.session_sha256 = hash(&"b".repeat(64));
    c.owner_sha256 = hash(&"a".repeat(64));
    let before = control_snapshot(&w);
    let p = w.prove_inventory_control(c.clone()).unwrap();
    assert_eq!(p.challenge, c);
    verify_control(&p);
    assert_eq!(control_snapshot(&w), before);
    #[cfg(unix)]
    if let Ok(path) = std::env::var("BTCX_TEST_PUBLIC_INVENTORY_VECTOR") {
        // Explicit test-only export of public signatures/evidence. Never export
        // keys, wallet databases, backups or private signing recovery records.
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let inventory = w
            .asset_inventory_proof(id, c.inputs.iter().map(|i| i.outpoint.clone()).collect())
            .unwrap();
        let value = serde_json::json!({"scope":"PUBLIC_NATIVE_BDK_STANDARD_SIGNATURE_VECTOR_LOCAL_GRAPH_NOT_LAB_OR_CORE", "proof":p,"inventory":inventory});
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)
            .unwrap();
        file.write_all(&serde_json::to_vec_pretty(&value).unwrap())
            .unwrap();
        file.sync_all().unwrap();
    }
}

#[test]
fn inventory_control_refuses_foreign_changed_stale_and_noncanonical_claims_before_returning_proof()
{
    for rgb in [false, true] {
        let (mut w, p, id) = fixture(true, false);
        let point = if rgb {
            p
        } else {
            coin(&mut w, KeychainKind::Internal, 10000, true, false)
        };
        let original = control_challenge(&w, &[point], rgb.then_some(id.as_str()));
        let before = control_snapshot(&w);
        for kind in [
            "network",
            "version",
            "purpose",
            "kind",
            "expired",
            "too_long",
            "expiry_encoding",
            "origin",
            "nonce",
            "tip_height",
            "tip_hash",
            "consignment",
            "foreign_point",
            "sats",
            "units",
            "script",
            "duplicate",
            "no_inputs",
            "asset",
            "amount_encoding",
            "overflow",
            "unknown",
        ] {
            let mut c = serde_json::to_value(&original).unwrap();
            match kind {
                "network" => c["network"] = "mainnet".into(),
                "version" => c["version"] = 2.into(),
                "purpose" => c["purpose"] = "approve-transaction".into(),
                "kind" => c["proof_kind"] = "external".into(),
                "expired" => c["expires_at"] = "1".into(),
                "too_long" => {
                    c["expires_at"] = (super::super::safe_bitcoin::unix_seconds().unwrap() + 120)
                        .to_string()
                        .into()
                }
                "expiry_encoding" => c["expires_at"] = "01".into(),
                "origin" => c["origin_sha256"] = "a\nHost:evil".into(),
                "nonce" => c["nonce"] = "D".repeat(64).into(),
                "tip_height" => c["tip_height"] = 199.into(),
                "tip_hash" => c["tip_hash"] = "e".repeat(64).into(),
                "consignment" => c["consignment_sha256"] = "e".repeat(64).into(),
                "foreign_point" => {
                    c["inputs"][0]["outpoint"] = format!("{}:0", "f".repeat(64)).into()
                }
                "sats" => c["inputs"][0]["input_sats"] = "1".into(),
                "units" => c["inputs"][0]["rgb_units"] = "2".into(),
                "script" => c["inputs"][0]["script_hex"] = format!("5120{}", "f".repeat(64)).into(),
                "duplicate" => {
                    let i = c["inputs"][0].clone();
                    c["inputs"].as_array_mut().unwrap().push(i);
                }
                "no_inputs" => c["inputs"] = serde_json::json!([]),
                "asset" => c["contract_id"] = "USDT".into(),
                "amount_encoding" => c["inputs"][0]["input_sats"] = "01".into(),
                "overflow" => c["inputs"][0]["input_sats"] = "18446744073709551616".into(),
                "unknown" => c["psbt"] = "must not reach signer".into(),
                _ => unreachable!(),
            }
            match serde_json::from_value::<InventoryControlChallenge>(c) {
                Ok(c) => assert!(
                    w.prove_inventory_control(c).is_err(),
                    "rgb={rgb} kind={kind}"
                ),
                Err(_) => assert_eq!(kind, "unknown"),
            }
            assert_eq!(control_snapshot(&w), before, "{kind}");
        }
    }
}

#[test]
fn inventory_control_witness_rejects_each_changed_context_and_another_wallet_key() {
    let mut w = wallet();
    let point = coin(&mut w, KeychainKind::Internal, 10000, true, false);
    let c = control_challenge(&w, &[point], None);
    let p = w.prove_inventory_control(c.clone()).unwrap();
    let witness: bitcoin::Witness =
        bitcoin::consensus::deserialize(&hex::decode(&p.proofs[0].witness_hex).unwrap()).unwrap();
    let address = BdkAddress::from_script(
        &bitcoin::ScriptBuf::from_hex(&c.inputs[0].script_hex).unwrap(),
        BdkNetwork::Regtest,
    )
    .unwrap();
    for field in [
        "kind",
        "origin",
        "session",
        "owner",
        "nonce",
        "expiry",
        "contract",
        "height",
        "hash",
        "consignment",
        "outpoint",
        "script",
        "sats",
        "units",
    ] {
        let mut bad = c.clone();
        match field {
            "kind" => bad.proof_kind = "rgb".into(),
            "origin" => bad.origin_sha256 = "e".repeat(64),
            "session" => bad.session_sha256 = "e".repeat(64),
            "owner" => bad.owner_sha256 = "e".repeat(64),
            "nonce" => bad.nonce = "e".repeat(64),
            "expiry" => bad.expires_at = "1".into(),
            "contract" => bad.contract_id = "USDT".into(),
            "height" => bad.tip_height = 1,
            "hash" => bad.tip_hash = "e".repeat(64),
            "consignment" => bad.consignment_sha256 = "e".repeat(64),
            "outpoint" => bad.inputs[0].outpoint = format!("{}:0", "e".repeat(64)),
            "script" => bad.inputs[0].script_hex = format!("5120{}", "e".repeat(64)),
            "sats" => bad.inputs[0].input_sats = "1".into(),
            "units" => bad.inputs[0].rgb_units = "1".into(),
            _ => unreachable!(),
        }
        assert!(
            bip322::verify_simple(&address, bad.message(&bad.inputs[0]), witness.clone()).is_err(),
            "{field}"
        );
    }
    let mut other = wallet();
    let op = coin(&mut other, KeychainKind::Internal, 10000, true, false);
    let other_challenge = control_challenge(&other, &[op], None);
    let other_proof = other.prove_inventory_control(other_challenge).unwrap();
    let foreign: bitcoin::Witness =
        bitcoin::consensus::deserialize(&hex::decode(&other_proof.proofs[0].witness_hex).unwrap())
            .unwrap();
    assert!(bip322::verify_simple(&address, c.message(&c.inputs[0]), foreign).is_err());
    assert!(other.prove_inventory_control(c).is_err());
}

#[test]
fn inventory_control_does_not_sign_bitcoin_that_acquires_rgb_or_pending_native_holds() {
    let mut w = wallet();
    let p = coin(&mut w, KeychainKind::External, 10000, true, false);
    let c = control_challenge(&w, &[p], None);
    assign_stock(&w, p, false);
    let before = control_snapshot(&w);
    assert!(w.prove_inventory_control(c).is_err());
    assert_eq!(control_snapshot(&w), before);

    let mut w = wallet();
    let p = coin(&mut w, KeychainKind::External, 10000, true, false);
    let c = control_challenge(&w, &[p], None);
    // Existing reservation-graph fixture; no preparation API, signer or network.
    let tx = BdkTransaction {
        version: bitcoin::transaction::Version::TWO,
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: vec![bitcoin::TxIn {
            previous_output: p,
            ..Default::default()
        }],
        output: vec![TxOut {
            value: BdkAmount::from_sat(9000),
            script_pubkey: w.bdk_wallet.get_utxo(p).unwrap().txout.script_pubkey,
        }],
    };
    let mut psbt = Psbt::from_unsigned_tx(tx).unwrap();
    psbt.inputs[0].witness_utxo = Some(w.bdk_wallet.get_utxo(p).unwrap().txout);
    retain_external(&mut w, &psbt, false);
    let before = control_snapshot(&w);
    assert!(w.prove_inventory_control(c).is_err());
    assert_eq!(control_snapshot(&w), before);
}
