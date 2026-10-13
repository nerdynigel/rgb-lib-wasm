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
    Applying,
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

impl PendingFill {
    /// Read-only binding used by the embedding consumer's exact-consent guard.
    /// A digest is not proof of approval, authentication or durable commit.
    pub fn digest(&self) -> &str {
        &self.digest
    }

    pub(crate) fn require_consented(&self) -> Result<(), String> {
        if self.state != State::Consented {
            return Err("pending fill must enter through consented state".to_owned());
        }
        Ok(())
    }
}

/// SAME-engine, memory-only application of the retained exact intent. This is a
/// Rust embedding seam, NOT approval or persistence. The authenticated consumer
/// must validate wallet-owned policy/ownership/receive authority before calling,
/// commit the complete resulting snapshot before any key operation, and refuse
/// the handle after every ambiguous result. No supplied record can replace the
/// retained intent. A partially attempted application remains Applying forever
/// until explicit reconciliation; restoring it does not grant retry authority.
#[cfg(feature = "esplora")]
impl super::Wallet {
    /// Apply once on the same stock; returns the exact reviewed PSBT without signing.
    /// `Applying` records require reconciliation and may never be retried.
    pub fn apply_consented_pending_fill(
        &mut self,
        association_id: &str,
        consent_digest: &str,
        now: u64,
    ) -> Result<String, crate::Error> {
        use super::rust_only::{AssetColoringInfo, ColoringInfo};
        use crate::bitcoin::{consensus, psbt::Psbt};
        use crate::{Error, FileContent};
        use std::{collections::HashMap, str::FromStr};
        let refusal = |details: &str| Error::Persistence {
            details: details.to_owned(),
        };
        if self.wallet_data.bitcoin_network != crate::BitcoinNetwork::Regtest {
            return Err(refusal("pending application is regtest-only"));
        }
        if !self.consumer_owned_persistence {
            return Err(refusal(
                "pending application requires consumer-owned persistence",
            ));
        }
        self.use_consumer_owned_persistence()?;
        let mut pending = self
            .btcx_pending_fill_v1
            .clone()
            .ok_or_else(|| refusal("pending consent absent"))?;
        let r = &pending.request;
        if pending.state != State::Consented
            || r.association_id != association_id
            || pending.digest != consent_digest
            || now >= r.expires_at
        {
            return Err(refusal(
                "pending application state/association/digest/expiry refused",
            ));
        }
        if r.psbt.len() > 16 * 1024 * 1024
            || r.colored_transaction_hex.len() > 8_000_000
            || r.consignment_bytes_hex.len() > 16_000_000
            || r.previous_transactions_hex
                .iter()
                .any(|tx| tx.len() > 8_000_000)
            || !(2..=4).contains(&r.allocations.len())
        {
            return Err(refusal("pending application size/allocation bounds"));
        }
        let original =
            Psbt::from_str(&r.psbt).map_err(|_| refusal("pending application PSBT invalid"))?;
        let mut psbt = original.clone();
        if original.inputs.len() != 2
            || original.unsigned_tx.input.len() != 2
            || original.unsigned_tx.input[0].previous_output.to_string() != r.spent_outpoint
            || original.unsigned_tx.input[0].previous_output
                == original.unsigned_tx.input[1].previous_output
            || original.inputs.iter().any(|i| {
                !i.partial_sigs.is_empty()
                    || i.final_script_sig.is_some()
                    || i.final_script_witness.is_some()
                    || i.tap_key_sig.is_some()
                    || !i.tap_script_sigs.is_empty()
            })
        {
            return Err(refusal(
                "pending application unsigned input binding invalid",
            ));
        }
        for (index, previous) in r.previous_transactions_hex.iter().enumerate() {
            let raw = hex::decode(previous)
                .map_err(|_| refusal("pending application previous transaction invalid"))?;
            let tx: crate::bitcoin::Transaction = consensus::deserialize(&raw)
                .map_err(|_| refusal("pending application previous transaction invalid"))?;
            let input = &original.unsigned_tx.input[index];
            if original.inputs[index].witness_utxo.is_none()
                || tx.compute_txid() != input.previous_output.txid
                || tx.output.get(input.previous_output.vout as usize)
                    != original.inputs[index].witness_utxo.as_ref()
            {
                return Err(refusal("pending application prevout binding invalid"));
            }
        }
        let colored = hex::decode(&r.colored_transaction_hex)
            .map_err(|_| refusal("pending application transaction invalid"))?;
        let consignment = hex::decode(&r.consignment_bytes_hex)
            .map_err(|_| refusal("pending application consignment invalid"))?;
        let facts = super::rust_only::inspect_transfer_offchain(
            &consignment,
            &colored,
            self.wallet_data.bitcoin_network,
        )?;
        if serde_json::to_value(&facts).map_err(|_| refusal("pending facts invalid"))?
            != serde_json::to_value(&pending.facts).map_err(|_| refusal("pending facts invalid"))?
            || facts.contract_id != r.contract_id
        {
            return Err(refusal("pending application inspected facts changed"));
        }
        let coloring = ColoringInfo {
            asset_info_map: HashMap::from([(
                crate::ContractId::from_str(&r.contract_id)
                    .map_err(|_| refusal("pending application contract invalid"))?,
                AssetColoringInfo {
                    output_map: r.allocations.iter().map(|(v, a)| (*v, a.amount)).collect(),
                    static_blinding: Some(r.asset_static_blinding),
                },
            )]),
            static_blinding: Some(r.static_blinding),
            nonce: Some(r.nonce),
        };
        // Recompute on THIS stock before consumption, comparing complete emitted
        // PSBT/transaction/consignment bytes. A shape/digest alone is insufficient.
        let check = |psbt: &Psbt, transfers: &[crate::RgbTransfer]| -> Result<(), Error> {
            if psbt.to_string() != pending.colored_psbt
                || hex::encode(consensus::serialize(&psbt.unsigned_tx)) != r.colored_transaction_hex
                || transfers.len() != 1
            {
                return Err(refusal("pending exact prospective transaction changed"));
            }
            let mut bytes = Vec::new();
            transfers[0]
                .save(&mut bytes)
                .map_err(|_| refusal("pending application transfer encoding failed"))?;
            if hex::encode(bytes) != r.consignment_bytes_hex {
                return Err(refusal("pending exact prospective transfer changed"));
            }
            Ok(())
        };
        let change = r
            .allocations
            .get(&r.change_vout)
            .ok_or_else(|| refusal("pending application change absent"))?;
        // The reviewed facts carry actual colored vouts. The uncolored request
        // may have had an OP_RETURN inserted at zero: never register its old vout.
        let shift = u32::from(
            !original
                .unsigned_tx
                .output
                .iter()
                .any(|o| o.script_pubkey.is_op_return())
                && original
                    .unsigned_tx
                    .output
                    .iter()
                    .any(|o| o.script_pubkey.is_p2tr()),
        );
        let change_vout = r
            .change_vout
            .checked_add(shift)
            .ok_or_else(|| refusal("pending application change vout overflow"))?;
        if pending.facts.allocations.len() != r.allocations.len() {
            return Err(refusal("pending application allocation count mismatch"));
        }
        let mut seen = std::collections::BTreeSet::new();
        for allocation in &pending.facts.allocations {
            let vout = allocation
                .vout
                .checked_sub(shift)
                .ok_or_else(|| refusal("pending application allocation vout invalid"))?;
            let expected = r
                .allocations
                .get(&vout)
                .ok_or_else(|| refusal("pending application extra allocation"))?;
            let output = original
                .unsigned_tx
                .output
                .get(vout as usize)
                .ok_or_else(|| refusal("pending application output absent"))?;
            if !seen.insert(vout)
                || expected.amount == 0
                || allocation.amount != expected.amount.to_string()
                || allocation.script_pubkey != expected.script_pubkey
                || hex::encode(output.script_pubkey.as_bytes()) != expected.script_pubkey
            {
                return Err(refusal("pending application allocation binding invalid"));
            }
        }
        let facts = pending
            .facts
            .allocations
            .iter()
            .filter(|a| a.vout == change_vout)
            .collect::<Vec<_>>();
        if facts.len() != 1
            || facts[0].amount != change.amount.to_string()
            || facts[0].script_pubkey != change.script_pubkey
        {
            return Err(refusal("pending application inspected change mismatch"));
        }
        // Poison before the first mutating engine call. Failures retain full
        // intent, including byte bindings, while forbidding a second consume.
        let mut applying = pending.clone();
        applying.state = State::Applying;
        self.btcx_pending_fill_v1 = Some(applying);
        let preview = self.color_psbt_and_export(&mut psbt, coloring.clone())?;
        check(&psbt, &preview)?;
        psbt = original;
        let transfers = self.color_psbt_and_consume_in_memory(&mut psbt, coloring)?;
        check(&psbt, &transfers)?;
        let row = self.register_collaborative_outgoing_transfer_in_memory(
            pending.facts.witness_txid.clone(),
            r.contract_id.clone(),
            vec![r.spent_outpoint.clone()],
            Some((change_vout, change.amount)),
            1,
        )?;
        if row.is_none() {
            return Err(refusal("pending application outgoing row absent"));
        }
        pending.state = State::Applied;
        self.btcx_pending_fill_v1 = Some(pending);
        Ok(psbt.to_string())
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
