//! IndexedDB persistence for WASM wallet snapshots.

use bdk_wallet::{ChangeSet, KeychainKind};
use rexie::{ObjectStore, Rexie, TransactionMode};
use serde::{Deserialize, Serialize};
use wasm_bindgen::JsValue;

use std::collections::HashMap;

use crate::database::memory_db::InMemoryDb;

const DB_NAME: &str = "rgb_lib_wallet";
const DB_VERSION: u32 = 1;
const STORE_NAME: &str = "snapshots";

/// A serializable snapshot of wallet state for IndexedDB persistence.
#[derive(Serialize, Deserialize)]
pub struct WalletSnapshot {
    /// Exact immutable candidate intent/consent, persisted with RGB state.
    /// No signing authority is inferred from this record.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "super::pending_fill::deserialize_optional"
    )]
    pub btcx_pending_fill_v1: Option<super::pending_fill::PendingFill>,
    /// Monotonic wallet-local snapshot sequence used to reject stale writes.
    pub sequence: u64,
    /// The in-memory RGB-lib database.
    pub db: InMemoryDb,
    /// The BDK wallet changeset.
    pub bdk_changeset: Option<ChangeSet>,
    /// Complete pending transfer state keyed by txid.
    pub(crate) transfer_artifacts: HashMap<String, super::online::TransferArtifacts>,
    /// Received consignment bytes keyed by recipient_id.
    #[serde(default)]
    pub received_consignments: HashMap<String, Vec<u8>>,
    /// Strict-encoded RGB Stock stash (base64).
    #[serde(default)]
    pub stock_stash_b64: Option<String>,
    /// Strict-encoded RGB Stock state (base64).
    #[serde(default)]
    pub stock_state_b64: Option<String>,
    /// Strict-encoded RGB Stock index (base64).
    #[serde(default)]
    pub stock_index_b64: Option<String>,
    /// Pinned derivation index per keychain for address reuse.
    #[serde(default)]
    pub reuse_address_index: HashMap<KeychainKind, u32>,
}

async fn open_db() -> Result<Rexie, String> {
    let rexie = Rexie::builder(DB_NAME)
        .version(DB_VERSION)
        .add_object_store(ObjectStore::new(STORE_NAME))
        .build()
        .await
        .map_err(|e| format!("IndexedDB open error: {e:?}"))?;
    Ok(rexie)
}

// A skipped stale write is a persistence failure, never an acknowledgement.
fn require_newer_snapshot(existing: u64, proposed: u64) -> Result<(), String> {
    if proposed <= existing {
        return Err("IndexedDB stale snapshot refused".to_owned());
    }
    Ok(())
}

/// Save a wallet snapshot to IndexedDB under the given key.
/// Resolves successfully only after this snapshot is committed. Equal/older
/// sequences refuse; callers must reconcile using authenticated durable state.
pub async fn save_snapshot(key: &str, snapshot: &WalletSnapshot) -> Result<(), String> {
    let db = open_db().await?;
    let tx = db
        .transaction(&[STORE_NAME], TransactionMode::ReadWrite)
        .map_err(|e| format!("IndexedDB transaction error: {e:?}"))?;
    let store = tx
        .store(STORE_NAME)
        .map_err(|e| format!("IndexedDB store error: {e:?}"))?;

    let json_str = serde_json::to_string(snapshot).map_err(|e| format!("Serialize error: {e}"))?;
    let js_key = JsValue::from_str(key);
    if let Some(existing) = store
        .get(js_key.clone())
        .await
        .map_err(|e| format!("IndexedDB get error: {e:?}"))?
    {
        let json = existing
            .as_string()
            .ok_or_else(|| "IndexedDB value is not a string".to_string())?;
        let existing: WalletSnapshot =
            serde_json::from_str(&json).map_err(|e| format!("Deserialize error: {e}"))?;
        require_newer_snapshot(existing.sequence, snapshot.sequence)?;
    }
    let js_val = JsValue::from_str(&json_str);

    store
        .put(&js_val, Some(&js_key))
        .await
        .map_err(|e| format!("IndexedDB put error: {e:?}"))?;
    tx.done()
        .await
        .map_err(|e| format!("IndexedDB commit error: {e:?}"))?;
    Ok(())
}

/// Serialize an RGB Stock into base64 components for snapshot persistence.
///
/// Returns `(stash_b64, state_b64, index_b64)` or `None` if serialization fails.
pub fn serialize_stock(stock: &rgbstd::persistence::Stock) -> Option<(String, String, String)> {
    use base64::{Engine as _, engine::general_purpose};
    use strict_encoding::StrictSerialize;

    let stash = stock
        .as_stash_provider()
        .to_strict_serialized::<{ u32::MAX as usize }>()
        .ok()?;
    let state = stock
        .as_state_provider()
        .to_strict_serialized::<{ u32::MAX as usize }>()
        .ok()?;
    let index = stock
        .as_index_provider()
        .to_strict_serialized::<{ u32::MAX as usize }>()
        .ok()?;

    Some((
        general_purpose::STANDARD.encode(stash.as_unconfined()),
        general_purpose::STANDARD.encode(state.as_unconfined()),
        general_purpose::STANDARD.encode(index.as_unconfined()),
    ))
}

/// Deserialize an RGB Stock from base64 components.
pub fn deserialize_stock(
    stash_b64: &str,
    state_b64: &str,
    index_b64: &str,
) -> Result<rgbstd::persistence::Stock, String> {
    use base64::{Engine as _, engine::general_purpose};
    use rgbstd::persistence::{MemIndex, MemStash, MemState};
    use strict_encoding::StrictDeserialize;

    const MAX: usize = u32::MAX as usize;

    let stash_bytes = general_purpose::STANDARD
        .decode(stash_b64)
        .map_err(|e| e.to_string())?;
    let state_bytes = general_purpose::STANDARD
        .decode(state_b64)
        .map_err(|e| e.to_string())?;
    let index_bytes = general_purpose::STANDARD
        .decode(index_b64)
        .map_err(|e| e.to_string())?;

    let stash = MemStash::from_strict_serialized::<MAX>(
        amplify::confinement::Confined::<Vec<u8>, 0, MAX>::try_from(stash_bytes)
            .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let state = MemState::from_strict_serialized::<MAX>(
        amplify::confinement::Confined::<Vec<u8>, 0, MAX>::try_from(state_bytes)
            .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let index = MemIndex::from_strict_serialized::<MAX>(
        amplify::confinement::Confined::<Vec<u8>, 0, MAX>::try_from(index_bytes)
            .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;

    Ok(rgbstd::persistence::Stock::with(stash, state, index))
}

// The ordinary Rexie builder is deliberately excluded: it installs an upgrade
// handler which creates stores. This request uses one connection and aborts any
// versionchange transaction before it can commit even an empty database.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(inline_js = r#"
export function readExistingSnapshot(databaseName, storeName, key, expectedVersion) {
  return new Promise((resolve, reject) => {
    let request;
    let blocked = false;
    try { request = globalThis.indexedDB.open(databaseName, expectedVersion); }
    catch (_) { reject('IndexedDB open failed'); return; }
    request.onupgradeneeded = () => {
      try { request.transaction.abort(); }
      catch (_) { reject('IndexedDB upgrade abort failed'); }
    };
    request.onerror = () => reject('IndexedDB existing database open failed');
    request.onblocked = () => { blocked = true; reject('IndexedDB existing database open blocked'); };
    request.onsuccess = () => {
      const db = request.result;
      if (blocked) { db.close(); return; }
      // A blocked request can eventually succeed; Promise settlement alone does
      // not cancel it. Still close it without opening a transaction.
      if (db.version !== expectedVersion || !db.objectStoreNames.contains(storeName)) {
        db.close(); reject('IndexedDB existing schema invalid'); return;
      }
      let tx;
      try {
        tx = db.transaction([storeName], 'readonly');
        let value;
        const get = tx.objectStore(storeName).get(key);
        get.onsuccess = () => { value = get.result; };
        get.onerror = () => { /* transaction abort supplies the failure */ };
        tx.onabort = () => { db.close(); reject('IndexedDB snapshot read aborted'); };
        tx.onerror = () => { db.close(); reject('IndexedDB snapshot read failed'); };
        tx.oncomplete = () => {
          db.close();
          if (typeof value !== 'string') reject('IndexedDB snapshot missing or invalid');
          else resolve(value);
        };
      } catch (_) {
        if (tx) { try { tx.abort(); } catch (_) {} }
        db.close(); reject('IndexedDB snapshot read failed');
      }
    };
  });
}
"#)]
extern "C" {
    #[wasm_bindgen::prelude::wasm_bindgen(catch, js_name = readExistingSnapshot)]
    async fn read_existing_snapshot(
        database_name: &str,
        store_name: &str,
        key: &str,
        expected_version: u32,
    ) -> Result<JsValue, JsValue>;
}

/// Read one existing record without creating/upgrading the database or writing.
/// This engine's snapshot format is JSON; encrypted consumer envelopes must be
/// authenticated by that consumer before calling `restore_existing_snapshot`.
#[cfg(target_arch = "wasm32")]
pub async fn read_existing_snapshot_bytes(key: &str) -> Result<String, String> {
    let value = read_existing_snapshot(DB_NAME, STORE_NAME, key, DB_VERSION)
        .await
        .map_err(|e| {
            e.as_string()
                .unwrap_or_else(|| "IndexedDB snapshot read failed".into())
        })?;
    value
        .as_string()
        .ok_or_else(|| "IndexedDB snapshot invalid".into())
}

/// Decode the engine's JSON format with strict storage semantics. Encrypted
/// consumers use `read_existing_snapshot_bytes`, authenticate their envelope,
/// deserialize, then call `Wallet::restore_existing_snapshot`.
#[cfg(target_arch = "wasm32")]
pub async fn load_existing_snapshot(key: &str) -> Result<WalletSnapshot, String> {
    let json = read_existing_snapshot_bytes(key).await?;
    serde_json::from_str(&json).map_err(|_| "Snapshot deserialization failed".into())
}

/// Load a wallet snapshot from IndexedDB for the given key.
pub async fn load_snapshot(key: &str) -> Result<Option<WalletSnapshot>, String> {
    let db = open_db().await?;
    let tx = db
        .transaction(&[STORE_NAME], TransactionMode::ReadOnly)
        .map_err(|e| format!("IndexedDB transaction error: {e:?}"))?;
    let store = tx
        .store(STORE_NAME)
        .map_err(|e| format!("IndexedDB store error: {e:?}"))?;

    let js_key = JsValue::from_str(key);
    let result = store
        .get(js_key)
        .await
        .map_err(|e| format!("IndexedDB get error: {e:?}"))?;

    match result {
        Some(js_val) => {
            let json_str = js_val
                .as_string()
                .ok_or_else(|| "IndexedDB value is not a string".to_string())?;
            let snapshot: WalletSnapshot =
                serde_json::from_str(&json_str).map_err(|e| format!("Deserialize error: {e}"))?;
            Ok(Some(snapshot))
        }
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    mod state_bytes {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/support/state_bytes.rs"
        ));
    }
    use super::*;
    use rgbstd::containers::ConsignmentExt;
    use rgbstd::contract::IssuerWrapper;
    use rgbstd::persistence::Stock;
    use schemata::NonInflatableAsset;

    #[test]
    fn stock_serialize_round_trip_empty() {
        let stock = Stock::in_memory();
        let (s, st, i) = serialize_stock(&stock).expect("serialize empty stock");

        let restored = deserialize_stock(&s, &st, &i).expect("deserialize empty stock");
        assert_eq!(restored.contracts().unwrap().count(), 0);
    }

    #[test]
    fn stock_serialize_round_trip_with_contract() {
        use crate::{AssetSpec, ContractTerms, DumbResolver, Precision, RicardianContract};
        use rgbstd::contract::ContractBuilder;
        use rgbstd::txout::BlindSeal;
        use rgbstd::{Amount, ChainNet, Identity, Txid as RgbTxid};

        let mut stock = Stock::in_memory();
        let empty_bytes = state_bytes::strict_stock_bytes(&serialize_stock(&stock).unwrap());

        let spec = AssetSpec {
            ticker: crate::Ticker::try_from("TEST".to_string()).unwrap(),
            name: crate::Name::try_from("Test Token".to_string()).unwrap(),
            details: None,
            precision: Precision::try_from(0u8).unwrap(),
        };
        let terms = ContractTerms {
            text: RicardianContract::default(),
            media: None,
        };

        // Dummy seal for the asset assignment
        let dummy_txid = "0000000000000000000000000000000000000000000000000000000000000000"
            .parse::<RgbTxid>()
            .unwrap();
        let seal = BlindSeal::new_random(dummy_txid, 0);

        let builder = ContractBuilder::with(
            Identity::default(),
            NonInflatableAsset::schema(),
            NonInflatableAsset::types(),
            NonInflatableAsset::scripts(),
            ChainNet::from(crate::BitcoinNetwork::Regtest),
        )
        .add_global_state("spec", spec)
        .expect("invalid spec")
        .add_global_state("terms", terms)
        .expect("invalid terms")
        .add_global_state("issuedSupply", Amount::from(1000u64))
        .expect("invalid issuedSupply")
        .add_fungible_state("assetOwner", seal, 1000u64)
        .expect("invalid assignment");

        let contract = builder.issue_contract().expect("issue contract");
        let contract_id = contract.contract_id();

        stock
            .import_contract(contract, &DumbResolver)
            .expect("import contract");

        // Verify contract exists before serialization
        assert!(stock.contracts().unwrap().any(|c| c.id == contract_id));

        // Serialize → JSON round-trip (simulates IndexedDB save/load)
        let (s, st, i) = serialize_stock(&stock).expect("serialize stock with contract");
        let stock_bytes = state_bytes::strict_stock_bytes(&(s.clone(), st.clone(), i.clone()));
        for (before, after) in empty_bytes.iter().zip(&stock_bytes) {
            assert_ne!(
                before, after,
                "import must change each strict stock component"
            );
        }

        let mut transfer_artifact = crate::wallet::online::TransferArtifacts::default();
        transfer_artifact
            .consignment_bytes
            .insert("asset-id".to_owned(), vec![1, 2, 3, 4]);
        transfer_artifact.signed_psbt = Some("signed-psbt".to_owned());
        let snapshot = WalletSnapshot {
            btcx_pending_fill_v1: None,
            sequence: 1,
            db: InMemoryDb::new(),
            bdk_changeset: None,
            transfer_artifacts: HashMap::from([("funding-txid".to_owned(), transfer_artifact)]),
            received_consignments: HashMap::new(),
            stock_stash_b64: Some(s),
            stock_state_b64: Some(st),
            stock_index_b64: Some(i),
            reuse_address_index: HashMap::new(),
        };

        // JSON round-trip (what IndexedDB does)
        let json = serde_json::to_string(&snapshot).expect("serialize snapshot to JSON");
        let restored_snapshot: WalletSnapshot =
            serde_json::from_str(&json).expect("deserialize snapshot from JSON");
        let restored_artifact = restored_snapshot
            .transfer_artifacts
            .get("funding-txid")
            .expect("restore pending transfer artifact");
        assert_eq!(
            restored_artifact.consignment_bytes.get("asset-id"),
            Some(&vec![1, 2, 3, 4])
        );
        assert_eq!(
            restored_artifact.signed_psbt.as_deref(),
            Some("signed-psbt")
        );

        let restored = deserialize_stock(
            restored_snapshot.stock_stash_b64.as_ref().unwrap(),
            restored_snapshot.stock_state_b64.as_ref().unwrap(),
            restored_snapshot.stock_index_b64.as_ref().unwrap(),
        )
        .expect("deserialize stock");

        let restored_bytes = state_bytes::strict_stock_bytes(&serialize_stock(&restored).unwrap());
        assert_eq!(
            restored_bytes, stock_bytes,
            "strict stock bytes must survive snapshot restore"
        );
        let snapshot_value = serde_json::to_value(&restored_snapshot).unwrap();
        assert_eq!(
            state_bytes::snapshot_stock_bytes(&snapshot_value),
            stock_bytes
        );
        println!(
            "THE876_BYTE_STATE {}",
            serde_json::json!({
                "boundary": "synthetic-contract-stock-round-trip",
                "stock": stock_bytes.each_ref().map(|b| state_bytes::receipt(b)),
                "snapshot_state": state_bytes::receipt(&state_bytes::snapshot_state_bytes(snapshot_value)),
                "serialized_snapshot": state_bytes::receipt(json.as_bytes())
            })
        );

        // Contract must survive the round-trip
        assert!(
            restored.contracts().unwrap().any(|c| c.id == contract_id),
            "Contract should exist after snapshot round-trip"
        );
    }
}

// Explicit encoded consumer persistence. Authentication/sequence checks belong
// to the consumer codec; exact old-record comparison prevents stale replacement.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(inline_js = r#"
export function commitEncodedSnapshot(databaseName, storeName, key, expected, encoded, version) {
  return new Promise((resolve, reject) => {
    let request, blocked = false;
    try { request = indexedDB.open(databaseName, version); }
    catch (_) { reject('IndexedDB encoded open failed'); return; }
    request.onupgradeneeded = event => {
      // Only explicit fresh setup may create a schema; never upgrade an existing one.
      if (event.oldVersion !== 0) { request.transaction.abort(); return; }
      request.result.createObjectStore(storeName);
    };
    request.onerror = () => reject('IndexedDB encoded open failed');
    request.onblocked = () => { blocked = true; reject('IndexedDB encoded open blocked'); };
    request.onsuccess = () => {
      const db = request.result;
      if (blocked) { db.close(); return; }
      if (db.version !== version || !db.objectStoreNames.contains(storeName)) {
        db.close(); reject('IndexedDB encoded schema invalid'); return;
      }
      let tx, failed = 'IndexedDB encoded transaction aborted';
      try {
        tx = db.transaction([storeName], 'readwrite');
        tx.onabort = () => { db.close(); reject(failed); };
        tx.onerror = () => { /* abort is terminal; do not report request success */ };
        tx.oncomplete = () => { db.close(); resolve(); };
        const store = tx.objectStore(storeName), count = store.count(key), get = store.get(key);
        get.onsuccess = () => {
          if (expected === null ? count.result !== 0 : count.result !== 1 || get.result !== expected) {
            failed = 'IndexedDB encoded record changed'; tx.abort(); return;
          }
          try { store.put(encoded, key); }
          catch (_) { failed = 'IndexedDB encoded put failed'; tx.abort(); }
        };
      } catch (_) {
        if (tx) { try { tx.abort(); } catch (_) {} }
        db.close(); reject('IndexedDB encoded transaction failed');
      }
    };
  });
}
"#)]
extern "C" {
    #[wasm_bindgen::prelude::wasm_bindgen(catch, js_name = commitEncodedSnapshot)]
    async fn commit_encoded_snapshot(
        database_name: &str,
        store_name: &str,
        key: &str,
        expected: JsValue,
        encoded: &str,
        version: u32,
    ) -> Result<JsValue, JsValue>;
}

/// Explicit codec-owned write, resolved only on transaction completion. None
/// means insert-only; Some requires byte-exact current-record equality inside
/// the SAME transaction as put. Never accepts arbitrary JS snapshot objects.
#[cfg(target_arch = "wasm32")]
pub async fn save_encoded_snapshot(
    key: &str,
    expected: Option<&str>,
    encoded: &str,
) -> Result<(), String> {
    let expected = expected.map(JsValue::from_str).unwrap_or(JsValue::NULL);
    commit_encoded_snapshot(DB_NAME, STORE_NAME, key, expected, encoded, DB_VERSION)
        .await
        .map_err(|e| {
            e.as_string()
                .unwrap_or_else(|| "IndexedDB encoded write failed".into())
        })?;
    Ok(())
}

#[cfg(test)]
mod stale_snapshot_tests {
    use super::require_newer_snapshot;

    #[test]
    fn stale_and_equal_snapshots_never_acknowledge_persistence() {
        for proposed in [0, 1, 40, 41] {
            assert!(require_newer_snapshot(41, proposed).is_err());
        }
        assert!(require_newer_snapshot(41, 42).is_ok());
        assert!(require_newer_snapshot(u64::MAX, u64::MAX).is_err());
    }
}
