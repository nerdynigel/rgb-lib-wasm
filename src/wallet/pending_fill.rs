//! Preservation of the candidate's exact pending intent inside engine state.
//! This type carries no signing authority. The facade must independently validate
//! policy, ownership, prevouts, inspection and lifecycle before confirmation/apply.
//! Expired intents remain readable for reconciliation; expiry is checked on use.
use crate::bitcoin::hashes::{Hash, sha256};
use crate::wallet::rust_only::InspectedTransfer;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    association_id: String,
    fill_id: String,
    terms_digest: String,
    expires_at: u64,
    psbt: String,
    previous_transactions_hex: [String; 2],
    contract_id: String,
    allocations: BTreeMap<u32, Allocation>,
    spent_outpoint: String,
    change_vout: u32,
    static_blinding: u64,
    asset_static_blinding: u64,
    nonce: u64,
    colored_transaction_hex: String,
    consignment_bytes_hex: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Allocation {
    amount: u64,
    script_pubkey: String,
}
/// Immutable persisted candidate intent, not a validated spending authorization.
#[derive(Clone, Serialize)]
pub struct PendingFill {
    request: Request,
    facts: InspectedTransfer,
    colored_psbt: String,
    digest: String,
    state: State,
}
#[derive(Clone, Serialize, Deserialize, PartialEq)]
enum State {
    Consented,
    Applied,
}

fn digest(request: &Request, facts: &InspectedTransfer, psbt: &str) -> Result<String, String> {
    let bytes = serde_json::to_vec(&("btcx-accounting-consent-v1", request, facts, psbt))
        .map_err(|_| "pending consent encoding failed".to_owned())?;
    Ok(sha256::Hash::hash(&bytes).to_string())
}

impl<'de> Deserialize<'de> for PendingFill {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Serialize, Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            request: Request,
            facts: InspectedTransfer,
            colored_psbt: String,
            digest: String,
            state: State,
        }
        let raw = serde_json::Value::deserialize(deserializer)?;
        let wire: Wire = serde_json::from_value(raw.clone()).map_err(serde::de::Error::custom)?;
        // InspectedTransfer is shared with other APIs. Refuse unknown nested
        // facts/allocation fields here without changing those APIs' decoders.
        if serde_json::to_value(&wire).map_err(serde::de::Error::custom)? != raw {
            return Err(serde::de::Error::custom("pending fill shape invalid"));
        }
        let expected = digest(&wire.request, &wire.facts, &wire.colored_psbt)
            .map_err(serde::de::Error::custom)?;
        if wire.digest != expected {
            return Err(serde::de::Error::custom("pending fill binding invalid"));
        }
        Ok(Self {
            request: wire.request,
            facts: wire.facts,
            colored_psbt: wire.colored_psbt,
            digest: wire.digest,
            state: wire.state,
        })
    }
}

/// Restore is not a lifecycle transition. Once an engine holds an exact pending
/// record, importing another snapshot may neither erase it nor replace its terms
/// or state. Reconciliation/confirmed application needs a separate guarded path.
/// This is preservation only; it does not authenticate either snapshot.
pub(crate) fn require_preserved_on_restore(
    current: Option<&PendingFill>,
    incoming: Option<&PendingFill>,
) -> Result<(), String> {
    let Some(current) = current else {
        return Ok(());
    };
    let incoming = incoming.ok_or_else(|| "pending fill erasure refused".to_owned())?;
    let current =
        serde_json::to_value(current).map_err(|_| "pending fill encoding failed".to_owned())?;
    let incoming =
        serde_json::to_value(incoming).map_err(|_| "pending fill encoding failed".to_owned())?;
    if incoming != current {
        return Err("pending fill replacement or state change refused".to_owned());
    }
    Ok(())
}

// Absence is legacy-compatible, but an explicitly present malformed/null record
// must never become absence and erase an ambiguous financial intent.
pub(crate) fn deserialize_optional<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<PendingFill>, D::Error> {
    PendingFill::deserialize(deserializer).map(Some)
}

#[cfg(test)]
pub(crate) fn storage_fixture() -> PendingFill {
    // Synthetic storage-only fixture. Never a valid RGB transfer or approval.
    let request = Request {
        association_id: "storage-case".into(),
        fill_id: "storage-fill".into(),
        terms_digest: "00".repeat(32),
        expires_at: 1,
        psbt: "storage-psbt".into(),
        previous_transactions_hex: ["01".into(), "02".into()],
        contract_id: "storage-contract".into(),
        allocations: BTreeMap::new(),
        spent_outpoint: "storage-seal".into(),
        change_vout: 2,
        static_blinding: 1,
        asset_static_blinding: 2,
        nonce: 3,
        colored_transaction_hex: "03".into(),
        consignment_bytes_hex: "04".into(),
    };
    let facts = InspectedTransfer {
        bytes_sha256: "00".repeat(32),
        contract_id: "storage-contract".into(),
        witness_transaction: vec![3],
        witness_txid: "storage-txid".into(),
        bundle_id: "storage-bundle".into(),
        anchor_json: "{}".into(),
        transition_id: "storage-transition".into(),
        allocations: vec![],
    };
    let colored_psbt = "storage-colored-psbt".to_owned();
    PendingFill {
        digest: digest(&request, &facts, &colored_psbt).unwrap(),
        request,
        facts,
        colored_psbt,
        state: State::Consented,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_record_roundtrips_and_expired_intent_remains_reconcilable() {
        let record = storage_fixture();
        let json = serde_json::to_string(&record).unwrap();
        let restored: PendingFill = serde_json::from_str(&json).unwrap();
        assert_eq!(serde_json::to_string(&restored).unwrap(), json);
    }
    #[test]
    fn changed_terms_and_digest_are_refused_on_decode() {
        let value = serde_json::to_value(storage_fixture()).unwrap();
        for field in [
            "association_id",
            "fill_id",
            "terms_digest",
            "contract_id",
            "psbt",
            "colored_transaction_hex",
            "consignment_bytes_hex",
            "spent_outpoint",
        ] {
            let mut changed = value.clone();
            changed["request"][field] = "changed".into();
            assert!(
                serde_json::from_value::<PendingFill>(changed).is_err(),
                "{field}"
            );
        }
        let mut changed = value.clone();
        changed["facts"]["witness_txid"] = "changed".into();
        assert!(serde_json::from_value::<PendingFill>(changed).is_err());
        let mut changed = value.clone();
        changed["digest"] = "changed".into();
        assert!(serde_json::from_value::<PendingFill>(changed).is_err());
        let mut changed = value;
        changed["state"] = "unknown".into();
        assert!(serde_json::from_value::<PendingFill>(changed).is_err());
    }
    #[test]
    fn unknown_record_and_request_fields_are_refused() {
        let value = serde_json::to_value(storage_fixture()).unwrap();
        let mut changed = value.clone();
        changed["other"] = true.into();
        assert!(serde_json::from_value::<PendingFill>(changed).is_err());
        let mut changed = value.clone();
        changed["facts"]["other"] = true.into();
        assert!(serde_json::from_value::<PendingFill>(changed).is_err());
        let mut changed = value;
        changed["request"]["other"] = true.into();
        assert!(serde_json::from_value::<PendingFill>(changed).is_err());
    }
}

#[cfg(test)]
pub(crate) fn replacement_storage_fixture() -> PendingFill {
    let mut record = storage_fixture();
    record.request.fill_id = "second-storage-fill".into();
    record.digest = digest(&record.request, &record.facts, &record.colored_psbt).unwrap();
    record
}
