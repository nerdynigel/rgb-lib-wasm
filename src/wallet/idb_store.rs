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

/// Older engines ignore unknown transfer metadata. A required wrapper makes
/// them refuse managed ordinary-send state instead of losing its attempt flags.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ManagedSnapshot {
    version: u8,
    managed_wallet_state: WalletSnapshot,
}

fn managed_transfers(snapshot: &WalletSnapshot) -> bool {
    snapshot.transfer_artifacts.values().any(|a| {
        a.external_send
            .as_ref()
            .is_some_and(|e| e.wallet_transfer.is_some())
    })
}

fn encode_snapshot(snapshot: &WalletSnapshot) -> Result<String, String> {
    if managed_transfers(snapshot) {
        #[derive(Serialize)]
        struct Envelope<'a> {
            version: u8,
            managed_wallet_state: &'a WalletSnapshot,
        }
        serde_json::to_string(&Envelope {
            version: 1,
            managed_wallet_state: snapshot,
        })
    } else {
        serde_json::to_string(snapshot)
    }
    .map_err(|_| "Wallet snapshot serialization failed".to_string())
}

fn decode_snapshot(json: &str) -> Result<WalletSnapshot, String> {
    let value: serde_json::Value =
        serde_json::from_str(json).map_err(|_| "Wallet snapshot decoding failed".to_string())?;
    if value.get("managed_wallet_state").is_some() || value.get("version").is_some() {
        let envelope: ManagedSnapshot = serde_json::from_value(value)
            .map_err(|_| "Unsupported managed wallet snapshot".to_string())?;
        if envelope.version != 1 {
            return Err("Unsupported managed wallet snapshot version".into());
        }
        Ok(envelope.managed_wallet_state)
    } else {
        let snapshot: WalletSnapshot = serde_json::from_value(value)
            .map_err(|_| "Wallet snapshot decoding failed".to_string())?;
        if managed_transfers(&snapshot) {
            return Err("Managed wallet state requires its versioned snapshot".into());
        }
        Ok(snapshot)
    }
}

/// A durable flush acknowledgement must never mean that a signing-attempt
/// snapshot was silently skipped. Managed originals are append-only even when
/// another tab has a newer sequence. Callers must reload/reconcile on failure.
fn snapshot_write_required(
    existing: &WalletSnapshot,
    candidate: &WalletSnapshot,
) -> Result<bool, String> {
    let refused = || "Managed wallet snapshot is stale or loses original obligations".to_string();
    if existing.sequence >= candidate.sequence {
        return if managed_transfers(existing) || managed_transfers(candidate) {
            Err(refused())
        } else {
            Ok(false)
        };
    }
    for (txid, old_artifact) in &existing.transfer_artifacts {
        let Some(old) = old_artifact
            .external_send
            .as_ref()
            .filter(|e| e.wallet_transfer.is_some())
        else {
            continue;
        };
        let new_artifact = candidate.transfer_artifacts.get(txid).ok_or_else(refused)?;
        let new = new_artifact.external_send.as_ref().ok_or_else(refused)?;
        let old_binding = old.wallet_transfer.as_ref().unwrap();
        let new_binding = new.wallet_transfer.as_ref().ok_or_else(refused)?;
        if old_binding.operation_id != new_binding.operation_id
            || old_binding.request_json != new_binding.request_json
            || (old_binding.signing_attempted && !new_binding.signing_attempted)
            || (old_binding.broadcast_attempted && !new_binding.broadcast_attempted)
            || (old_binding.restored && !new_binding.restored)
            || old.prepared_psbt != new.prepared_psbt
            || old.expected_tx_hex != new.expected_tx_hex
            || old.fascia_json != new.fascia_json
            || old.recipients_digest != new.recipients_digest
            || old_artifact.consignment_bytes != new_artifact.consignment_bytes
            || (old_artifact.signed_psbt.is_some()
                && old_artifact.signed_psbt != new_artifact.signed_psbt)
            || (old.conflict_txid.is_some() && old.conflict_txid != new.conflict_txid)
        {
            return Err(refused());
        }
        use super::online::ExternalSendStatus;
        match old.status {
            ExternalSendStatus::Prepared => {}
            ExternalSendStatus::BroadcastRecorded => {
                if !matches!(
                    new.status,
                    ExternalSendStatus::BroadcastRecorded | ExternalSendStatus::Finalized
                ) {
                    return Err(refused());
                }
            }
            ExternalSendStatus::Finalized | ExternalSendStatus::Conflicted => {
                if new.status != old.status {
                    return Err(refused());
                }
            }
        }
    }
    for (txid, artifact) in &candidate.transfer_artifacts {
        let Some(binding) = artifact
            .external_send
            .as_ref()
            .and_then(|e| e.wallet_transfer.as_ref())
        else {
            continue;
        };
        if (artifact.signed_psbt.is_some() && !binding.signing_attempted)
            || (binding.broadcast_attempted && artifact.signed_psbt.is_none())
            || existing.transfer_artifacts.get(txid).is_some_and(|a| {
                a.external_send
                    .as_ref()
                    .is_none_or(|e| e.wallet_transfer.is_none())
            })
        {
            return Err(refused());
        }
    }
    Ok(true)
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

/// Save a wallet snapshot to IndexedDB under the given key.
pub async fn save_snapshot(key: &str, snapshot: &WalletSnapshot) -> Result<(), String> {
    let db = open_db().await?;
    let tx = db
        .transaction(&[STORE_NAME], TransactionMode::ReadWrite)
        .map_err(|e| format!("IndexedDB transaction error: {e:?}"))?;
    let store = tx
        .store(STORE_NAME)
        .map_err(|e| format!("IndexedDB store error: {e:?}"))?;

    let json_str = encode_snapshot(snapshot)?;
    let js_key = JsValue::from_str(key);
    if let Some(existing) = store
        .get(js_key.clone())
        .await
        .map_err(|e| format!("IndexedDB get error: {e:?}"))?
    {
        let json = existing
            .as_string()
            .ok_or_else(|| "IndexedDB value is not a string".to_string())?;
        let existing = decode_snapshot(&json)?;
        if !snapshot_write_required(&existing, snapshot)? {
            tx.done()
                .await
                .map_err(|e| format!("IndexedDB commit error: {e:?}"))?;
            return Ok(());
        }
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
            let snapshot = decode_snapshot(&json_str)?;
            Ok(Some(snapshot))
        }
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rgbstd::containers::ConsignmentExt;
    use rgbstd::contract::IssuerWrapper;
    use rgbstd::persistence::Stock;
    use schemata::NonInflatableAsset;

    #[test]
    fn managed_snapshot_blocks_old_readers_and_preserves_original_attempts() {
        let mut snapshot = WalletSnapshot {
            sequence: u64::MAX,
            db: InMemoryDb::new(),
            bdk_changeset: None,
            transfer_artifacts: HashMap::new(),
            received_consignments: HashMap::new(),
            stock_stash_b64: None,
            stock_state_b64: None,
            stock_index_b64: None,
            reuse_address_index: HashMap::new(),
        };
        let legacy = encode_snapshot(&snapshot).unwrap();
        assert!(serde_json::from_str::<WalletSnapshot>(&legacy).is_ok());
        assert_eq!(decode_snapshot(&legacy).unwrap().sequence, u64::MAX);
        snapshot.sequence = 7;
        let artifact = serde_json::from_value(serde_json::json!({
            "batch_info": null, "asset_infos": {}, "consignment_bytes": {"asset":[1,2,3]},
            "signed_psbt": "original-signed-bytes", "external_send": {
                "prepared_psbt": "original-prepared-bytes", "expected_tx_hex":"00",
                "fascia_json":"{}", "recipients_digest":"original-digest", "status":"Prepared",
                "conflict_txid":null, "wallet_transfer":{
                    "operation_id":"01a19805-0462-759b-9050-b9b0feb9da8f",
                    "request_json":"immutable-request", "signing_attempted":true, "broadcast_attempted":true
                }
            }
        })).unwrap();
        snapshot
            .transfer_artifacts
            .insert("original-txid".into(), artifact);
        let encoded = encode_snapshot(&snapshot).unwrap();
        assert!(serde_json::from_str::<WalletSnapshot>(&encoded).is_err());
        let restored = decode_snapshot(&encoded).unwrap();
        assert_eq!(restored.sequence, snapshot.sequence);
        assert_eq!(
            serde_json::to_value(&restored.transfer_artifacts).unwrap(),
            serde_json::to_value(&snapshot.transfer_artifacts).unwrap()
        );
        let mut unknown: serde_json::Value = serde_json::from_str(&encoded).unwrap();
        unknown["version"] = serde_json::json!(2);
        assert!(decode_snapshot(&unknown.to_string()).is_err());
        unknown["version"] = serde_json::json!(1);
        unknown["db"] = serde_json::to_value(&snapshot.db).unwrap();
        assert!(decode_snapshot(&unknown.to_string()).is_err());
        assert!(decode_snapshot(&serde_json::to_string(&snapshot).unwrap()).is_err());

        // A skipped stale write cannot acknowledge a pre-signing flush.
        let existing = decode_snapshot(&encoded).unwrap();
        let mut candidate = decode_snapshot(&encoded).unwrap();
        assert!(snapshot_write_required(&existing, &candidate).is_err());
        candidate.sequence += 1;
        assert_eq!(snapshot_write_required(&existing, &candidate), Ok(true));
        let original = candidate.transfer_artifacts["original-txid"].clone();
        candidate.transfer_artifacts.clear();
        assert!(snapshot_write_required(&existing, &candidate).is_err());
        candidate
            .transfer_artifacts
            .insert("original-txid".into(), original.clone());
        candidate
            .transfer_artifacts
            .get_mut("original-txid")
            .unwrap()
            .signed_psbt = None;
        assert!(snapshot_write_required(&existing, &candidate).is_err());
        candidate
            .transfer_artifacts
            .insert("original-txid".into(), original.clone());
        candidate
            .transfer_artifacts
            .get_mut("original-txid")
            .unwrap()
            .consignment_bytes
            .clear();
        assert!(snapshot_write_required(&existing, &candidate).is_err());
        candidate
            .transfer_artifacts
            .insert("original-txid".into(), original);
        candidate
            .transfer_artifacts
            .get_mut("original-txid")
            .unwrap()
            .external_send
            .as_mut()
            .unwrap()
            .wallet_transfer
            .as_mut()
            .unwrap()
            .broadcast_attempted = false;
        assert!(snapshot_write_required(&existing, &candidate).is_err());
        let mut restored_existing = decode_snapshot(&encoded).unwrap();
        restored_existing
            .transfer_artifacts
            .get_mut("original-txid")
            .unwrap()
            .external_send
            .as_mut()
            .unwrap()
            .wallet_transfer
            .as_mut()
            .unwrap()
            .restored = true;
        let mut candidate = decode_snapshot(&encoded).unwrap();
        candidate.sequence += 1;
        assert!(snapshot_write_required(&restored_existing, &candidate).is_err());
        candidate
            .transfer_artifacts
            .get_mut("original-txid")
            .unwrap()
            .external_send
            .as_mut()
            .unwrap()
            .wallet_transfer
            .as_mut()
            .unwrap()
            .restored = true;
        assert_eq!(
            snapshot_write_required(&restored_existing, &candidate),
            Ok(true)
        );
    }

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

        let mut transfer_artifact = crate::wallet::online::TransferArtifacts::default();
        transfer_artifact
            .consignment_bytes
            .insert("asset-id".to_owned(), vec![1, 2, 3, 4]);
        transfer_artifact.signed_psbt = Some("signed-psbt".to_owned());
        let snapshot = WalletSnapshot {
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
        let json = encode_snapshot(&snapshot).expect("serialize snapshot to JSON");
        let restored_snapshot = decode_snapshot(&json).expect("deserialize snapshot from JSON");
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

        // Contract must survive the round-trip
        assert!(
            restored.contracts().unwrap().any(|c| c.id == contract_id),
            "Contract should exist after snapshot round-trip"
        );
    }
}
