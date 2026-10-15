//! Real local importer regression; no network, persistence or settlement proof.
use super::*;
use crate::bitcoin::{
    Amount, OutPoint, Sequence, Transaction, TxIn, TxOut, Witness, absolute, psbt::Psbt,
    transaction,
};
use crate::wallet::rust_only::{AssetColoringInfo, ColoringInfo};

#[test]
fn local_receipt_metadata_is_bound_to_existing_receive_event() {
    let (seller, funding) = Wallet::new_funded_regtest_wallet_for_tests().unwrap();
    let contract = seller.list_assets(vec![]).unwrap().nia.unwrap()[0]
        .asset_id
        .clone();
    let mut buyer = super::super::offline::address_reuse_tests::make_test_wallet(false);
    *buyer.rgb_stock.borrow_mut() = Some(rgbstd::persistence::Stock::in_memory());
    let receive = buyer
        .witness_receive_in_memory(
            None,
            Assignment::Fungible(1000),
            Some(600),
            vec!["rpc://127.0.0.1:3000/json-rpc".into()],
            1,
        )
        .unwrap();
    let script = script_buf_from_recipient_id(receive.recipient_id.clone())
        .unwrap()
        .unwrap();
    let mut psbt = Psbt::from_unsigned_tx(Transaction {
        version: transaction::Version::TWO,
        lock_time: absolute::LockTime::ZERO,
        input: vec![TxIn {
            previous_output: OutPoint::new(funding.compute_txid(), 0),
            script_sig: ScriptBuf::new(),
            sequence: Sequence::MAX,
            witness: Witness::new(),
        }],
        output: vec![
            TxOut {
                value: Amount::from_sat(600),
                script_pubkey: script.clone(),
            },
            TxOut {
                value: Amount::from_sat(300),
                script_pubkey: ScriptBuf::new(),
            },
        ],
    })
    .unwrap();
    psbt.inputs[0].witness_utxo = Some(funding.output[0].clone());
    let transfers = seller
        .color_psbt_and_export(
            &mut psbt,
            ColoringInfo {
                asset_info_map: HashMap::from([(
                    ContractId::from_str(&contract).unwrap(),
                    AssetColoringInfo {
                        output_map: HashMap::from([(0, 1000), (1, 107)]),
                        static_blinding: Some(1),
                    },
                )]),
                static_blinding: Some(1),
                nonce: Some(1),
            },
        )
        .unwrap();
    let receiving_vout = psbt
        .unsigned_tx
        .output
        .iter()
        .position(|o| o.script_pubkey == script)
        .unwrap() as u32;
    let mut bytes = Vec::new();
    transfers[0].save(&mut bytes).unwrap();
    // Establish an older receiving event BEFORE either consumer. No result or
    // accepted financial authority is injected; both imports use the real engine.
    let mut initial = serde_json::to_value(buyer.snapshot_for_review().unwrap()).unwrap();
    initial["db"]["batch_transfers"][0]["created_at"] = serde_json::json!(1);
    let data = buyer.wallet_data.clone();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let mut results = Vec::new();
    for _ in 0..2 {
        let mut candidate = Wallet::restore_existing_snapshot(
            data.clone(),
            serde_json::from_value(initial.clone()).unwrap(),
        )
        .unwrap();
        runtime
            .block_on(candidate.import_witness_consignment_in_memory(
                receive.recipient_id.clone(),
                general_purpose::STANDARD.encode(&bytes),
                psbt.unsigned_tx.compute_txid().to_string(),
                receiving_vout,
            ))
            .unwrap();
        let result = serde_json::to_value(candidate.snapshot_for_review().unwrap()).unwrap();
        assert_eq!(result["db"]["assets"][0]["added_at"], 1);
        assert_eq!(
            result["db"]["batch_transfers"][0]["status"],
            "WaitingConfirmations"
        );
        results.push(result);
    }
    // Complete state, including metadata/stock/sequence, must agree unchanged.
    assert_eq!(results[0], results[1]);
}
