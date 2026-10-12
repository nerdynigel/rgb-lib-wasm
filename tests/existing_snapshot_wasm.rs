//! Fresh disposable, unfunded real IndexedDB proof. Never uses external services.
use rgb_lib_wasm::wallet::{AssetFilter, DatabaseType, WalletData};
use rgb_lib_wasm::{AssetSchema, BitcoinNetwork, Wallet, generate_keys};
use wasm_bindgen::prelude::*;
use wasm_bindgen_test::*;
wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen(inline_js = r#"
const name = 'rgb_lib_wallet';
export async function resetDb(version, store) {
  await new Promise((resolve, reject) => {
    const r = indexedDB.deleteDatabase(name);
    r.onsuccess = resolve; r.onerror = reject; r.onblocked = reject;
  });
  if (!version) return;
  await new Promise((resolve, reject) => {
    const r = indexedDB.open(name, version);
    r.onupgradeneeded = () => { if (store) r.result.createObjectStore(store); };
    r.onsuccess = () => { r.result.close(); resolve(); };
    r.onerror = reject;
  });
}
export async function record(key, value, write) {
  const db = await new Promise((resolve, reject) => {
    const r = indexedDB.open(name); r.onsuccess = () => resolve(r.result); r.onerror = reject;
  });
  return new Promise((resolve, reject) => {
    const tx = db.transaction(['snapshots'], write ? 'readwrite' : 'readonly');
    let result;
    const r = write ? tx.objectStore('snapshots').put(value, key) : tx.objectStore('snapshots').get(key);
    r.onsuccess = () => { result = r.result; };
    tx.oncomplete = () => { db.close(); resolve(result); };
    tx.onabort = () => { db.close(); reject('test transaction abort'); };
  });
}
export async function schema() {
  return new Promise((resolve, reject) => {
    const r = indexedDB.open(name);
    r.onupgradeneeded = () => r.transaction.abort();
    r.onerror = () => resolve('absent');
    r.onsuccess = () => {
      const d = r.result; const s = d.version + ':' + [...d.objectStoreNames].join(',');
      d.close(); resolve(s);
    };
  });
}
let getOriginal, txOriginal, writes = 0, reads = 0;
export function observe(mode) {
  writes = 0; reads = 0;
  getOriginal = IDBObjectStore.prototype.get;
  txOriginal = IDBDatabase.prototype.transaction;
  IDBDatabase.prototype.transaction = function(...args) {
    if (args[1] !== 'readonly') { writes++; throw new Error('unexpected write'); }
    reads++;
    return txOriginal.apply(this, args);
  };
  IDBObjectStore.prototype.get = function(...args) {
    if (mode === 'throw') throw new Error('injected get failure');
    const r = getOriginal.apply(this, args);
    if (mode === 'abort') this.transaction.abort();
    return r;
  };
}
export function unobserve() {
  IDBObjectStore.prototype.get = getOriginal;
  IDBDatabase.prototype.transaction = txOriginal;
  return JSON.stringify({writes, reads});
}
"#)]
extern "C" {
    #[wasm_bindgen(catch, js_name = resetDb)]
    async fn reset_db(version: u32, store: &str) -> Result<JsValue, JsValue>;
    #[wasm_bindgen(catch)]
    async fn record(key: &str, value: JsValue, write: bool) -> Result<JsValue, JsValue>;
    #[wasm_bindgen(catch)]
    async fn schema() -> Result<JsValue, JsValue>;
    fn observe(mode: &str);
    fn unobserve() -> String;
}
fn data() -> WalletData {
    let k = generate_keys(BitcoinNetwork::Regtest);
    WalletData {
        data_dir: ":memory:strict-test".into(),
        bitcoin_network: BitcoinNetwork::Regtest,
        database_type: DatabaseType::Sqlite,
        max_allocations_per_utxo: 5,
        account_xpub_vanilla: k.account_xpub_vanilla,
        account_xpub_colored: k.account_xpub_colored,
        mnemonic: None,
        master_fingerprint: k.master_fingerprint,
        vanilla_keychain: None,
        supported_schemas: vec![AssetSchema::Nia],
        reuse_addresses: false,
    }
}
async fn refuse(d: &WalletData, mode: &str, expected_reads: u64) {
    observe(mode);
    let result = Wallet::load_existing_snapshot(d.clone()).await;
    let counts: serde_json::Value = serde_json::from_str(&unobserve()).unwrap();
    assert!(result.is_err());
    assert_eq!(counts["writes"], 0);
    assert_eq!(counts["reads"], expected_reads);
}
#[wasm_bindgen_test]
async fn strict_existing_snapshot_real_idb() {
    let d = data();
    let key = format!("{}/{}", d.data_dir, d.master_fingerprint);
    reset_db(0, "").await.unwrap();
    refuse(&d, "", 0).await;
    assert_eq!(schema().await.unwrap().as_string().unwrap(), "absent");
    reset_db(2, "sentinel").await.unwrap();
    refuse(&d, "", 0).await;
    assert_eq!(schema().await.unwrap().as_string().unwrap(), "2:sentinel");
    reset_db(1, "sentinel").await.unwrap();
    refuse(&d, "", 0).await;
    assert_eq!(schema().await.unwrap().as_string().unwrap(), "1:sentinel");
    reset_db(1, "snapshots").await.unwrap();
    refuse(&d, "", 1).await;
    for value in [
        JsValue::from_f64(3.0),
        JsValue::from_str("{bad"),
        JsValue::from_str("{}"),
    ] {
        record(&key, value.clone(), true).await.unwrap();
        refuse(&d, "", 1).await;
        assert_eq!(record(&key, JsValue::NULL, false).await.unwrap(), value);
    }
    reset_db(1, "snapshots").await.unwrap();
    // Explicit setup only. Strict load never invokes new/flush.
    let wallet = Wallet::new(d.clone()).unwrap();
    wallet.flush().await.unwrap();
    let raw = record(&key, JsValue::NULL, false)
        .await
        .unwrap()
        .as_string()
        .unwrap();
    drop(wallet);
    refuse(&d, "throw", 1).await;
    refuse(&d, "abort", 1).await;
    assert_eq!(
        record(&key, JsValue::NULL, false)
            .await
            .unwrap()
            .as_string()
            .unwrap(),
        raw
    );
    let baseline: serde_json::Value = serde_json::from_str(&raw).unwrap();
    for field in [
        "stock_stash_b64",
        "stock_state_b64",
        "stock_index_b64",
        "bdk_changeset",
    ] {
        let mut invalid = baseline.clone();
        invalid[field] = serde_json::Value::Null;
        let invalid_raw = serde_json::to_string(&invalid).unwrap();
        record(&key, JsValue::from_str(&invalid_raw), true)
            .await
            .unwrap();
        refuse(&d, "", 1).await;
        assert_eq!(
            record(&key, JsValue::NULL, false)
                .await
                .unwrap()
                .as_string()
                .unwrap(),
            invalid_raw
        );
    }
    let mut invalid = baseline.clone();
    invalid["stock_state_b64"] = "invalid!".into();
    let invalid_raw = serde_json::to_string(&invalid).unwrap();
    record(&key, JsValue::from_str(&invalid_raw), true)
        .await
        .unwrap();
    refuse(&d, "", 1).await;
    assert_eq!(
        record(&key, JsValue::NULL, false)
            .await
            .unwrap()
            .as_string()
            .unwrap(),
        invalid_raw
    );
    record(&key, JsValue::from_str(&raw), true).await.unwrap();
    let foreign = data();
    let snapshot = rgb_lib_wasm::wallet::idb_store::load_existing_snapshot(&key)
        .await
        .unwrap();
    assert!(Wallet::restore_existing_snapshot(foreign, snapshot).is_err());
    observe("");
    let restored = Wallet::load_existing_snapshot(d).await.unwrap();
    assert!(
        restored
            .list_assets(vec![AssetSchema::Nia])
            .unwrap()
            .nia
            .unwrap()
            .is_empty()
    );
    assert!(
        restored
            .list_transfers(AssetFilter::Any, None)
            .unwrap()
            .is_empty()
    );
    drop(restored);
    assert_eq!(unobserve(), r#"{"writes":0,"reads":1}"#);
    assert_eq!(
        record(&key, JsValue::NULL, false)
            .await
            .unwrap()
            .as_string()
            .unwrap(),
        raw
    );
    use sha2::{Digest, Sha256};
    console_log!(
        "STRICT_SNAPSHOT real_idb cases=16 writes=0 raw_bytes={} sha256={}",
        raw.len(),
        hex::encode(Sha256::digest(raw.as_bytes()))
    );
    // Runner destroys this disposable browser profile. The legacy setup flush
    // keeps a Rexie connection open; do not change that unrelated default path.
}

#[wasm_bindgen(inline_js = r#"
let originalPut;
export function abortEncodedPut(enable) {
  if (!enable) { IDBObjectStore.prototype.put = originalPut; return; }
  originalPut = IDBObjectStore.prototype.put;
  IDBObjectStore.prototype.put = function(...args) {
    const request = originalPut.apply(this, args);
    // Proves a successful put request does not mean durable commit.
    request.addEventListener('success', () => this.transaction.abort());
    return request;
  };
}
"#)]
extern "C" {
    #[wasm_bindgen(js_name = abortEncodedPut)]
    fn abort_put(enable: bool);
}

#[wasm_bindgen_test]
async fn encoded_snapshot_commit_real_idb() {
    use rgb_lib_wasm::wallet::idb_store::{read_existing_snapshot_bytes, save_encoded_snapshot};
    let d = data();
    let wallet = Wallet::new(d.clone()).unwrap();
    let key = wallet.idb_key();
    let snapshot = wallet.snapshot_for_persistence().unwrap();
    let raw = serde_json::to_string(&snapshot).unwrap();
    let restored = Wallet::restore_existing_snapshot(d, snapshot).unwrap();
    assert_eq!(
        restored
            .list_transfers(AssetFilter::Any, None)
            .unwrap()
            .len(),
        0
    );
    // The seam itself never creates storage or schedules a background save.
    assert_eq!(schema().await.unwrap().as_string().unwrap(), "absent");
    save_encoded_snapshot(&key, None, &raw).await.unwrap();
    assert_eq!(read_existing_snapshot_bytes(&key).await.unwrap(), raw);
    // A committed stored undefined is an existing corrupt record, not absence.
    record("undefined-record", JsValue::UNDEFINED, true)
        .await
        .unwrap();
    assert!(
        save_encoded_snapshot("undefined-record", None, "replacement")
            .await
            .is_err()
    );
    assert!(
        record("undefined-record", JsValue::NULL, false)
            .await
            .unwrap()
            .is_undefined()
    );
    assert!(
        save_encoded_snapshot(&key, None, "replacement")
            .await
            .is_err()
    );
    assert!(
        save_encoded_snapshot(&key, Some("foreign"), "replacement")
            .await
            .is_err()
    );
    assert_eq!(read_existing_snapshot_bytes(&key).await.unwrap(), raw);
    abort_put(true);
    let failed = save_encoded_snapshot(&key, Some(&raw), "aborted").await;
    abort_put(false);
    assert!(failed.is_err());
    assert_eq!(read_existing_snapshot_bytes(&key).await.unwrap(), raw);
    save_encoded_snapshot(&key, Some(&raw), "committed")
        .await
        .unwrap();
    assert_eq!(
        read_existing_snapshot_bytes(&key).await.unwrap(),
        "committed"
    );
    reset_db(1, "sentinel").await.unwrap();
    assert!(save_encoded_snapshot(&key, None, "new").await.is_err());
    assert_eq!(schema().await.unwrap().as_string().unwrap(), "1:sentinel");
    reset_db(2, "sentinel").await.unwrap();
    assert!(save_encoded_snapshot(&key, None, "new").await.is_err());
    assert_eq!(schema().await.unwrap().as_string().unwrap(), "2:sentinel");
    console_log!(
        "ENCODED_SNAPSHOT actual engine serialization/restore and real-IDB commit/CAS/abort/schema PASS; unfunded only"
    );
}
