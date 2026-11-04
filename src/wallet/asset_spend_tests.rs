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
    assert!(w.asset_spend_view(id).unwrap().inputs.is_empty());
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
