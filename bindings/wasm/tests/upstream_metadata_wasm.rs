//! UTEXO #114: a metadata call must never reveal the signing mnemonic.
use rgb_lib_wasm::wallet::{DatabaseType, WalletData};
use rgb_lib_wasm::{AssetSchema, BitcoinNetwork, generate_keys};
use rgb_lib_wasm_bindings::WasmWallet;
use wasm_bindgen_test::*;
wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
fn upstream_public_metadata_has_null_mnemonic_and_preserves_identity() {
    let keys = generate_keys(BitcoinNetwork::Regtest);
    let data = WalletData {
        data_dir: "/upstream-metadata".into(),
        bitcoin_network: BitcoinNetwork::Regtest,
        database_type: DatabaseType::Sqlite,
        max_allocations_per_utxo: 5,
        account_xpub_vanilla: keys.account_xpub_vanilla,
        account_xpub_colored: keys.account_xpub_colored,
        mnemonic: Some(keys.mnemonic.clone()),
        master_fingerprint: keys.master_fingerprint.clone(),
        vanilla_keychain: None,
        supported_schemas: vec![AssetSchema::Nia],
        reuse_addresses: false,
    };
    let wallet = WasmWallet::new(&serde_json::to_string(&data).unwrap()).unwrap();
    let public = wallet.get_wallet_data().unwrap();
    assert!(
        js_sys::Reflect::get(&public, &"mnemonic".into())
            .unwrap()
            .is_null()
    );
    let text = js_sys::JSON::stringify(&public)
        .unwrap()
        .as_string()
        .unwrap();
    assert!(!text.contains(&keys.mnemonic));
    // Public JS metadata is not a restore payload: optional fields may be
    // undefined. Inspect the public identity without reconstructing keys.
    let decoded: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(decoded["masterFingerprint"], data.master_fingerprint);
    assert_eq!(decoded["accountXpubColored"], data.account_xpub_colored);
}
