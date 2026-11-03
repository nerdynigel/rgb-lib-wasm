//! Native one-attempt partial signing. Existing BDK/secp256k1 implementations
//! own keys and BIP341; this module binds their result to a durable exact PSBT.
use super::*;
use bitcoin::key::TapTweak;
use bitcoin::secp256k1::{Message, Secp256k1, XOnlyPublicKey};
use bitcoin::sighash::{Prevouts, SighashCache, TapSighashType};

/// Wallet-local explicit signing request, never a market approval credential.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CollaborativePartialRequest {
    /// This native request format is version one.
    pub version: u8,
    /// Only the isolated REGTEST profile is implemented.
    pub network: String,
    /// The exact unsigned PSBT independently reviewed in the wallet.
    pub psbt: String,
    /// Complete canonical ASCII-sorted wallet-owned input set.
    pub own_outpoints: Vec<String>,
    /// Bounded local approval expiry, canonical integer Unix seconds.
    pub expires_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PartialSigningRecord {
    pub(crate) request: CollaborativePartialRequest,
    pub(crate) attempted: bool,
    pub(crate) restored: bool,
}

fn refused() -> Error {
    Error::InvalidPsbt { details: "Collaborative original or complete native input ownership could not be verified; preserve its original obligation".into() }
}

fn parse_request(
    request: &CollaborativePartialRequest,
) -> Result<(Psbt, HashSet<OutPoint>, u64), Error> {
    if request.version != 1
        || request.network != "regtest"
        || request.psbt.len() > 180_000
        || !(1..=32).contains(&request.own_outpoints.len())
    {
        return Err(refused());
    }
    let expiry: u64 = request.expires_at.parse().map_err(|_| refused())?;
    if expiry.to_string() != request.expires_at || expiry == 0 || expiry > 9_007_199_254_740_991 {
        return Err(refused());
    }
    let mut owned = HashSet::new();
    for (i, value) in request.own_outpoints.iter().enumerate() {
        let point = OutPoint::from_str(value).map_err(|_| refused())?;
        if point.to_string() != *value
            || (i > 0 && request.own_outpoints[i - 1] >= *value)
            || !owned.insert(point)
        {
            return Err(refused());
        }
    }
    let psbt = Psbt::from_str(&request.psbt)?;
    if psbt.to_string() != request.psbt
        || psbt.version != 0
        || psbt.unsigned_tx.version.0 != 2
        || psbt.unsigned_tx.lock_time.to_consensus_u32() != 0
        || !(2..=64).contains(&psbt.inputs.len())
        || psbt.inputs.len() != psbt.unsigned_tx.input.len()
        || !(1..=128).contains(&psbt.outputs.len())
        || owned.len() >= psbt.inputs.len()
        || psbt.unknown.iter().any(|(key, value)| {
            key.type_value == 6 && (!key.key.is_empty() || value.as_slice() != [0])
        })
    {
        return Err(refused());
    }
    let mut seen = HashSet::new();
    let mut input_sats = 0u64;
    for (i, input) in psbt.inputs.iter().enumerate() {
        let txin = &psbt.unsigned_tx.input[i];
        let prevout = input.witness_utxo.as_ref().ok_or_else(refused)?;
        if !seen.insert(txin.previous_output)
            || txin.sequence.to_consensus_u32() != u32::MAX
            || !txin.script_sig.is_empty()
            || !txin.witness.is_empty()
            || !prevout.script_pubkey.is_p2tr()
            || prevout.value.to_sat() == 0
            || prevout.value > BdkAmount::MAX_MONEY
            || input
                .sighash_type
                .is_some_and(|s| ![0, 1].contains(&s.to_u32()))
            || !input.partial_sigs.is_empty()
            || input.tap_key_sig.is_some()
            || !input.tap_script_sigs.is_empty()
            || input.final_script_sig.is_some()
            || input.final_script_witness.is_some()
            || !input.tap_scripts.is_empty()
            || input.tap_merkle_root.is_some()
            || input.redeem_script.is_some()
            || input.witness_script.is_some()
        {
            return Err(refused());
        }
        if let Some(previous) = &input.non_witness_utxo {
            if previous.compute_txid() != txin.previous_output.txid
                || previous.output.get(txin.previous_output.vout as usize) != Some(prevout)
            {
                return Err(refused());
            }
        }
        input_sats = input_sats
            .checked_add(prevout.value.to_sat())
            .ok_or_else(refused)?;
    }
    if !owned.is_subset(&seen) || input_sats > BdkAmount::MAX_MONEY.to_sat() {
        return Err(refused());
    }
    let output_sats = psbt.unsigned_tx.output.iter().try_fold(0u64, |sum, o| {
        sum.checked_add(o.value.to_sat()).ok_or_else(refused)
    })?;
    let fee = input_sats.checked_sub(output_sats).ok_or_else(refused)?;
    if !(1..=10_000).contains(&fee) {
        return Err(refused());
    }
    Ok((psbt, owned, expiry))
}

pub(in crate::wallet) fn retained_partial_inputs(
    txid: &str,
    record: &PartialSigningRecord,
) -> Result<HashSet<OutPoint>, Error> {
    let (psbt, points, _) = parse_request(&record.request)?;
    if !record.attempted || psbt.unsigned_tx.compute_txid().to_string() != txid {
        return Err(refused());
    }
    Ok(points)
}

/// Established BIP341 verification, including all prevouts and every owned key
/// signature. Foreign input maps and the exact transaction remain unchanged.
fn verify_partial(request: &CollaborativePartialRequest, signed: &Psbt) -> Result<(), Error> {
    let (original, owned, _) = parse_request(request)?;
    if original.unsigned_tx != signed.unsigned_tx
        || original.inputs.len() != signed.inputs.len()
        || original.version != signed.version
        || original.xpub != signed.xpub
        || original.proprietary != signed.proprietary
        || original.unknown != signed.unknown
    {
        return Err(refused());
    }
    let prevouts = original
        .inputs
        .iter()
        .map(|i| i.witness_utxo.clone().ok_or_else(refused))
        .collect::<Result<Vec<_>, _>>()?;
    let secp = Secp256k1::verification_only();
    let mut cache = SighashCache::new(&original.unsigned_tx);
    for (i, actual) in signed.inputs.iter().enumerate() {
        if !owned.contains(&original.unsigned_tx.input[i].previous_output) {
            if actual != &original.inputs[i] {
                return Err(refused());
            }
            continue;
        }
        let sig = match (&actual.tap_key_sig, &actual.final_script_witness) {
            (Some(sig), None) => *sig,
            (None, Some(w)) if w.len() == 1 => {
                bitcoin::taproot::Signature::from_slice(w.iter().next().ok_or_else(refused)?)
                    .map_err(|_| refused())?
            }
            _ => return Err(refused()),
        };
        if !matches!(
            sig.sighash_type,
            TapSighashType::Default | TapSighashType::All
        ) || original.inputs[i]
            .sighash_type
            .is_some_and(|s| s.to_u32() != sig.sighash_type as u32)
        {
            return Err(refused());
        }
        let key = XOnlyPublicKey::from_slice(&prevouts[i].script_pubkey.as_bytes()[2..])
            .map_err(|_| refused())?;
        let mut metadata = actual.clone();
        metadata.tap_key_sig = None;
        metadata.final_script_witness = None;
        let before = &original.inputs[i];
        if before.tap_internal_key.is_none()
            && let Some(internal) = metadata.tap_internal_key
        {
            let (output_key, _) = internal.tap_tweak(&secp, None);
            if output_key.to_x_only_public_key() != key {
                return Err(refused());
            }
            metadata.tap_internal_key = None;
            if !before.tap_key_origins.contains_key(&internal)
                && metadata
                    .tap_key_origins
                    .get(&internal)
                    .is_some_and(|(leaves, _)| leaves.is_empty())
            {
                metadata.tap_key_origins.remove(&internal);
            }
        }
        if &metadata != before {
            return Err(refused());
        }
        let digest = cache
            .taproot_key_spend_signature_hash(i, &Prevouts::All(&prevouts), sig.sighash_type)
            .map_err(|_| refused())?;
        secp.verify_schnorr(
            &sig.signature,
            &Message::from_digest(digest.to_byte_array()),
            &key,
        )
        .map_err(|_| refused())?;
    }
    Ok(())
}

impl Wallet {
    fn collaborative_partial_preflight(
        &self,
        request: &CollaborativePartialRequest,
        continuing: bool,
        now_unix: u64,
    ) -> Result<Psbt, Error> {
        let (psbt, own, expiry) = parse_request(request)?;
        if self.bitcoin_network() != BitcoinNetwork::Regtest
            || expiry <= now_unix
            || expiry > now_unix.checked_add(300).ok_or_else(refused)?
        {
            return Err(refused());
        }
        let txid = psbt.unsigned_tx.compute_txid().to_string();
        let artifact = self.transfer_artifacts.get(&txid);
        if continuing {
            let record = artifact
                .and_then(|a| a.partial_signing.as_ref())
                .ok_or_else(refused)?;
            if record.request != *request
                || !record.attempted
                || record.restored
                || artifact.unwrap().signed_psbt.is_some()
            {
                return Err(refused());
            }
        } else if artifact.is_some_and(|a| {
            a.partial_signing.is_some() || a.signed_psbt.is_some() || a.external_send.is_none()
        }) {
            return Err(refused());
        }
        let mut assigned = BTreeMap::new();
        if let Some(external) = artifact.and_then(|a| a.external_send.as_ref()) {
            let batch = self.batch_transfer_by_txid(&txid)?.ok_or_else(refused)?;
            if external.wallet_transfer.is_some()
                || external.restored_for_partial
                || external.status != ExternalSendStatus::Prepared
                || batch.status != TransferStatus::Initiated
                || external.prepared_psbt != request.psbt
                || external.expected_tx_hex
                    != Self::tx_stripped_hex_from_unsigned(&psbt.unsigned_tx)
            {
                return Err(refused());
            }
            let (asset, amount) = artifact
                .unwrap()
                .asset_infos
                .iter()
                .next()
                .map(|(id, i)| (id.clone(), i.original_assignments_needed.fungible))
                .ok_or_else(refused)?;
            if self.database.check_asset_exists(asset.clone())?.schema != AssetSchema::Nia {
                return Err(refused());
            }
            let fascia: Fascia =
                serde_json::from_str(&external.fascia_json).map_err(|_| refused())?;
            if fascia.witness_id().to_string() != txid {
                return Err(refused());
            }
            let consignment = artifact
                .unwrap()
                .consignment_bytes
                .get(&asset)
                .ok_or_else(refused)?;
            self.inspect_external_transfer(consignment, txid.clone(), asset.clone())?;
            assigned = self.external_rgb_original_input_units(&txid, &asset, amount)?;
            self.assert_wallet_rgb_stock_input_units(&asset, &psbt, &assigned)?;
            if assigned.keys().any(|p| !request.own_outpoints.contains(p)) {
                return Err(refused());
            }
        }
        let free = self
            .bitcoin_partial_input_view(&txid)?
            .inputs
            .into_iter()
            .map(|i| (i.outpoint.clone(), i))
            .collect::<HashMap<_, _>>();
        let db = self.database.get_db_data(false)?;
        let holds = self.bitcoin_partial_input_holds(&db, &txid)?;
        let tip = self.bdk_wallet.latest_checkpoint();
        for (i, txin) in psbt.unsigned_tx.input.iter().enumerate() {
            let previous = psbt.inputs[i].witness_utxo.as_ref().ok_or_else(refused)?;
            let is_owned = self.bdk_wallet.is_mine(previous.script_pubkey.clone());
            if own.contains(&txin.previous_output) != is_owned {
                return Err(refused());
            }
            if !is_owned {
                // Preserve BDK's full previous-transaction requirement. Foreign
                // derivations are masked while signing and restored byte-exact.
                if psbt.inputs[i].non_witness_utxo.is_none() {
                    return Err(refused());
                }
                continue;
            }
            let native = self
                .bdk_wallet
                .get_utxo(txin.previous_output)
                .ok_or_else(refused)?;
            let ChainPosition::Confirmed { anchor, .. } = native.chain_position else {
                return Err(refused());
            };
            if native.is_spent
                || native.txout != *previous
                || anchor.block_id.height > tip.height()
                || holds.contains(&txin.previous_output)
            {
                return Err(refused());
            }
            let prior = self
                .bdk_wallet
                .get_tx(txin.previous_output.txid)
                .ok_or_else(refused)?;
            if prior.tx_node.tx.is_coinbase()
                && u64::from(tip.height()) - u64::from(anchor.block_id.height) + 1 < 100
            {
                return Err(refused());
            }
            if !assigned.contains_key(&txin.previous_output.to_string()) {
                let qualified = free
                    .get(&txin.previous_output.to_string())
                    .ok_or_else(refused)?;
                if qualified.amount_sats != previous.value.to_sat().to_string()
                    || qualified.script_hex != previous.script_pubkey.to_hex_string()
                {
                    return Err(refused());
                }
            }
        }
        Ok(psbt)
    }

    /// Read-only actual Native evidence for one independently reviewed PSBT.
    /// Prepared RGB seals are classified from their exact existing original,
    /// not presented as free balance or reconstructed from market amounts.
    pub fn review_collaborative_partial(
        &self,
        request: CollaborativePartialRequest,
    ) -> Result<serde_json::Value, Error> {
        let psbt = self.collaborative_partial_preflight(
            &request,
            false,
            super::safe_bitcoin::unix_seconds()?,
        )?;
        let txid = psbt.unsigned_tx.compute_txid().to_string();
        let mut asset = None;
        let mut assigned = BTreeMap::new();
        if let Some(artifact) = self.transfer_artifacts.get(&txid)
            && artifact.external_send.is_some()
        {
            let (id, info) = artifact.asset_infos.iter().next().ok_or_else(refused)?;
            assigned = self.external_rgb_original_input_units(
                &txid,
                id,
                info.original_assignments_needed.fungible,
            )?;
            asset = Some(id.clone());
        }
        let mut inputs = Vec::new();
        for point in &request.own_outpoints {
            let output = self
                .bdk_wallet
                .get_utxo(OutPoint::from_str(point).map_err(|_| refused())?)
                .ok_or_else(refused)?;
            inputs.push(serde_json::json!({"outpoint":point,"input_sats":output.txout.value.to_sat().to_string(),"rgb_units":assigned.get(point).copied().unwrap_or(0).to_string(),"script_hex":output.txout.script_pubkey.to_hex_string()}));
        }
        let owned = psbt
            .unsigned_tx
            .output
            .iter()
            .filter(|o| self.bdk_wallet.is_mine(o.script_pubkey.clone()))
            .map(|o| o.script_pubkey.to_hex_string())
            .collect::<std::collections::BTreeSet<_>>();
        let tip = self.bdk_wallet.latest_checkpoint();
        Ok(
            serde_json::json!({"version":1,"kind":"wallet-collaborative-inputs","network":"regtest","txid":txid,"contract_id":asset,"tip_height":tip.height(),"tip_hash":tip.hash().to_string(),"inputs":inputs,"owned_output_scripts":owned}),
        )
    }

    async fn require_collaborative_original_unobserved(&self, txid: &str) -> Result<(), Error> {
        if self
            .indexer()
            .get_tx_with_status(&Txid::from_str(txid).map_err(|_| refused())?)
            .await?
            .is_some()
        {
            return Err(refused());
        }
        Ok(())
    }

    fn sign_collaborative_owned_inputs(
        &self,
        request: &CollaborativePartialRequest,
        psbt: &mut Psbt,
    ) -> Result<(), Error> {
        let (_, own, _) = parse_request(request)?;
        let mut foreign = Vec::new();
        for (i, txin) in psbt.unsigned_tx.input.iter().enumerate() {
            if own.contains(&txin.previous_output) {
                continue;
            }
            foreign.push((i, psbt.inputs[i].clone()));
            // An attacker can supply origins for an as-yet-untracked wallet
            // address. Do not let those hints authorize a foreign input. This
            // adapter leaves every transaction/prevout byte untouched.
            psbt.inputs[i].bip32_derivation.clear();
            psbt.inputs[i].tap_key_origins.clear();
            psbt.inputs[i].tap_internal_key = None;
        }
        let signing = self._sign_psbt(
            psbt,
            Some(SignOptions {
                try_finalize: false,
                ..Default::default()
            }),
        );
        let mut overreach = false;
        for (i, original) in foreign {
            let actual = &psbt.inputs[i];
            overreach |= actual.tap_key_sig.is_some()
                || !actual.tap_script_sigs.is_empty()
                || !actual.partial_sigs.is_empty()
                || actual.final_script_witness.is_some()
                || actual.final_script_sig.is_some();
            psbt.inputs[i] = original;
        }
        if overreach {
            return Err(refused());
        }
        signing
    }

    /// Exact local authorization only. Persists one attempt before BDK, then
    /// persists the original partial before returning. Never posts or broadcasts.
    pub async fn sign_collaborative_partial(
        &mut self,
        online: Online,
        request: CollaborativePartialRequest,
    ) -> Result<serde_json::Value, Error> {
        self._check_xprv()?;
        let clock = super::safe_bitcoin::unix_seconds;
        self.collaborative_partial_preflight(&request, false, clock()?)?;
        self.require_wallet_rgb_regtest(&online).await?;
        let psbt = self.collaborative_partial_preflight(&request, false, clock()?)?;
        let txid = psbt.unsigned_tx.compute_txid().to_string();
        self.require_collaborative_original_unobserved(&txid)
            .await?;
        if let Some(artifact) = self.transfer_artifacts.get(&txid) {
            for bytes in artifact.consignment_bytes.values() {
                self.validate_external_transfer_ancestry(bytes, txid.clone())
                    .await?;
            }
        }
        self.transfer_artifacts
            .entry(txid.clone())
            .or_default()
            .partial_signing = Some(PartialSigningRecord {
            request: request.clone(),
            attempted: true,
            restored: false,
        });
        self.flush().await?;
        let mut psbt = self.collaborative_partial_preflight(&request, true, clock()?)?;
        self.require_collaborative_original_unobserved(&txid)
            .await?;
        self.collaborative_partial_preflight(&request, true, clock()?)?;
        let signing = self.sign_collaborative_owned_inputs(&request, &mut psbt);
        self.transfer_artifacts.get_mut(&txid).unwrap().signed_psbt = Some(psbt.to_string());
        self.flush().await?;
        signing?;
        verify_partial(&request, &psbt)?;
        if request.expires_at.parse::<u64>().map_err(|_| refused())? <= clock()? {
            return Err(refused());
        }
        self.collaborative_partial_data(txid)
    }

    /// Read only the exact retained original; no expiry grants a new signature.
    pub fn collaborative_partial_data(&self, txid: String) -> Result<serde_json::Value, Error> {
        if txid.len() != 64 || Txid::from_str(&txid).map_err(|_| refused())?.to_string() != txid {
            return Err(refused());
        }
        let genesis =
            bitcoin::blockdata::constants::genesis_block(BdkNetwork::Regtest).block_hash();
        if self.bitcoin_network() != BitcoinNetwork::Regtest
            || self
                .bdk_wallet
                .latest_checkpoint()
                .get(0)
                .is_none_or(|c| c.hash() != genesis)
        {
            return Err(refused());
        }
        let artifact = self.transfer_artifacts.get(&txid).ok_or_else(refused)?;
        let record = artifact.partial_signing.as_ref().ok_or_else(refused)?;
        retained_partial_inputs(&txid, record)?;
        let signed = artifact.signed_psbt.as_deref();
        let complete = signed
            .and_then(|v| Psbt::from_str(v).ok())
            .is_some_and(|v| verify_partial(&record.request, &v).is_ok());
        Ok(
            serde_json::json!({"version":1,"kind":"wallet-collaborative-partial","scope":"RETAINED_PARTIAL_NOT_SETTLEMENT","network":"regtest","txid":txid,"request":record.request,"attempted":record.attempted,"restored":record.restored,"own_signatures_complete":complete,"signed_psbt":signed}),
        )
    }
}

#[cfg(test)]
#[path = "collaborative_partial_tests.rs"]
pub(in crate::wallet) mod tests;
