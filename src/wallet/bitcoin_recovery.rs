//! Complete-set Bitcoin self-recovery, using existing BDK/Bitcoin primitives.
//! This state is never an RGB receipt, market approval or reservation release.
use super::*;

#[path = "bitcoin_input_preparation.rs"]
pub(in crate::wallet) mod input_preparation;
pub use input_preparation::{BitcoinInputPreparation, BitcoinInputPreparationRequest};

/// Exact wallet-local self-recovery approval. No key or secret is a field.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BitcoinRecoveryRequest {
    /// Request version, currently one.
    pub version: u8,
    /// Only isolated REGTEST is supported.
    pub network: String,
    /// Exact independently reviewed unsigned PSBT, at most32inputs and1output.
    pub psbt: String,
    /// Canonical whole satoshis per vbyte, 1–1000.
    pub fee_rate: String,
    /// Canonical maximum fee in sats, 1–10000.
    pub maximum_fee_sats: String,
    /// One bounded signing approval in canonical Unix seconds.
    pub expires_at: String,
    /// Explicit separate wallet-owned output preparation; legacy recovery
    /// requests omit this field and keep their original one-output semantics.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_preparation: Option<BitcoinInputPreparation>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BitcoinRecoveryRecord {
    pub(crate) request: BitcoinRecoveryRequest,
    pub(crate) attempted: bool,
    pub(crate) broadcast_attempted: bool,
    pub(crate) restored: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub(crate) cancelled: bool,
}

fn is_false(value: &bool) -> bool {
    !*value
}

fn refused() -> Error {
    Error::InvalidPsbt {
        details: "Exact Native Bitcoin self-recovery original required; preserve all held inputs"
            .into(),
    }
}
fn integer(value: &str) -> Result<u64, Error> {
    let n = value.parse::<u64>().map_err(|_| refused())?;
    if n.to_string() != value {
        return Err(refused());
    }
    Ok(n)
}
fn parse_request(
    request: &BitcoinRecoveryRequest,
) -> Result<(Psbt, HashSet<OutPoint>, u64), Error> {
    if request.input_preparation.is_some() {
        return input_preparation::parse_preparation(request);
    }
    let rate = integer(&request.fee_rate)?;
    let cap = integer(&request.maximum_fee_sats)?;
    let expiry = integer(&request.expires_at)?;
    if request.version != 1
        || request.network != "regtest"
        || request.psbt.len() > 180_000
        || !(1..=1000).contains(&rate)
        || !(1..=10000).contains(&cap)
        || expiry == 0
    {
        return Err(refused());
    }
    let psbt = Psbt::from_str(&request.psbt).map_err(|_| refused())?;
    if psbt.to_string() != request.psbt
        || psbt.version != 0
        || !psbt.xpub.is_empty()
        || !psbt.proprietary.is_empty()
        || !psbt.unknown.is_empty()
        || !(1..=32).contains(&psbt.inputs.len())
        || psbt.inputs.len() != psbt.unsigned_tx.input.len()
        || psbt.outputs.len() != 1
        || psbt.unsigned_tx.output.len() != 1
        || psbt.outputs[0] != bitcoin::psbt::Output::default()
        || psbt.unsigned_tx.version.0 != 2
        || psbt.unsigned_tx.lock_time.to_consensus_u32() != 0
    {
        return Err(refused());
    }
    let mut points = HashSet::new();
    let mut previous = None;
    let mut sats = 0u64;
    for (txin, input) in psbt.unsigned_tx.input.iter().zip(&psbt.inputs) {
        let outpoint = txin.previous_output.to_string();
        let witness = input.witness_utxo.as_ref().ok_or_else(refused)?;
        let expected = bitcoin::psbt::Input {
            witness_utxo: Some(witness.clone()),
            ..Default::default()
        };
        if previous.as_ref().is_some_and(|p: &String| p >= &outpoint)
            || !points.insert(txin.previous_output)
            || txin.sequence != bitcoin::Sequence::MAX
            || !txin.script_sig.is_empty()
            || !txin.witness.is_empty()
            || *input != expected
            || !witness.script_pubkey.is_p2tr()
            || witness.value.to_sat() == 0
            || witness.value > BdkAmount::MAX_MONEY
        {
            return Err(refused());
        }
        previous = Some(outpoint);
        sats = sats
            .checked_add(witness.value.to_sat())
            .ok_or_else(refused)?;
    }
    let output = &psbt.unsigned_tx.output[0];
    let fee = sats
        .checked_sub(output.value.to_sat())
        .ok_or_else(refused)?;
    let weight = (bitcoin::consensus::serialize(&psbt.unsigned_tx).len() as u64)
        .checked_mul(4)
        .and_then(|n| n.checked_add(2 + 67 * psbt.inputs.len() as u64))
        .ok_or_else(refused)?;
    let maximum_vbytes = weight.checked_add(3).ok_or_else(refused)? / 4;
    if sats > BdkAmount::MAX_MONEY.to_sat()
        || !output.script_pubkey.is_p2tr()
        || output.value < TxOut::minimal_non_dust(output.script_pubkey.clone()).value
        || fee == 0
        || fee > cap
        || maximum_vbytes.checked_mul(rate) != Some(fee)
    {
        return Err(refused());
    }
    Ok((psbt, points, expiry))
}
pub(in crate::wallet) fn retained_bitcoin_recovery_inputs(
    txid: &str,
    record: &BitcoinRecoveryRecord,
) -> Result<HashSet<OutPoint>, Error> {
    let (psbt, points, _) = parse_request(&record.request)?;
    if (!record.attempted && record.request.input_preparation.is_none())
        || psbt.unsigned_tx.compute_txid().to_string() != txid
        || (record.cancelled
            && (record.request.input_preparation.is_none()
                || record.attempted
                || record.broadcast_attempted))
    {
        return Err(refused());
    }
    Ok(if record.cancelled {
        HashSet::new()
    } else {
        points
    })
}
/// Validate new-format artifacts before restore/write. A failed signer may have
/// retained incomplete bytes; they still bind the exact transaction and must
/// never be dropped or re-signed. Dispatch requires every signature to verify.
pub(in crate::wallet) fn validate_bitcoin_recovery_artifact(
    txid: &str,
    artifact: &TransferArtifacts,
) -> Result<(), Error> {
    let Some(record) = &artifact.bitcoin_recovery else {
        return Ok(());
    };
    retained_bitcoin_recovery_inputs(txid, record)?;
    if (artifact.signed_psbt.is_some() && (!record.attempted || record.cancelled))
        || (record.broadcast_attempted && !record.attempted)
    {
        return Err(refused());
    }
    if artifact.external_send.is_some()
        || artifact.external_receive.is_some()
        || artifact.partial_signing.is_some()
        || artifact.batch_info.is_some()
        || !artifact.asset_infos.is_empty()
        || !artifact.consignment_bytes.is_empty()
    {
        return Err(refused());
    }
    if let Some(encoded) = &artifact.signed_psbt {
        if encoded.len() > 180000 {
            return Err(refused());
        }
        let original = parse_request(&record.request)?.0;
        let signed = Psbt::from_str(encoded).map_err(|_| refused())?;
        if signed.unsigned_tx != original.unsigned_tx {
            return Err(refused());
        }
    }
    if record.broadcast_attempted {
        verify_original(record, artifact.signed_psbt.as_deref().ok_or_else(refused)?)?;
    }
    Ok(())
}
fn verify_original(record: &BitcoinRecoveryRecord, encoded: &str) -> Result<Psbt, Error> {
    let (original, points, _) = parse_request(&record.request)?;
    if encoded.len() > 180000 {
        return Err(refused());
    }
    let signed = Psbt::from_str(encoded).map_err(|_| refused())?;
    super::collaborative_partial::verify_owned_keyspends(&original, &signed, &points)?;
    let tx = signed.clone().extract_tx().map_err(|_| refused())?;
    if tx.input.iter().any(|i| i.witness.is_empty()) {
        return Err(refused());
    }
    Ok(signed)
}

impl Wallet {
    fn bitcoin_recovery_preflight(
        &self,
        request: &BitcoinRecoveryRequest,
        after_attempt: bool,
        now: u64,
    ) -> Result<Psbt, Error> {
        let (psbt, _, expiry) = parse_request(request)?;
        if expiry <= now || expiry > now.checked_add(120).ok_or_else(refused)? {
            return Err(refused());
        }
        let txid = psbt.unsigned_tx.compute_txid().to_string();
        if after_attempt {
            let artifact = self.transfer_artifacts.get(&txid).ok_or_else(refused)?;
            let record = artifact.bitcoin_recovery.as_ref().ok_or_else(refused)?;
            if record.request != *request
                || record.restored
                || record.broadcast_attempted
                || !record.attempted
                || record.cancelled
                || artifact.signed_psbt.is_some()
            {
                return Err(refused());
            }
        } else if let Some(artifact) = self.transfer_artifacts.get(&txid) {
            let record = artifact.bitcoin_recovery.as_ref().ok_or_else(refused)?;
            if request.input_preparation.is_none()
                || record.request != *request
                || record.attempted
                || record.broadcast_attempted
                || record.restored
                || record.cancelled
                || artifact.signed_psbt.is_some()
            {
                return Err(refused());
            }
        }
        if request.input_preparation.is_some() {
            self.check_bitcoin_preparation_outputs(request, &psbt)?;
        } else if !matches!(
            self.bdk_wallet
                .derivation_of_spk(psbt.unsigned_tx.output[0].script_pubkey.clone()),
            Some((KeychainKind::Internal, _))
        ) {
            return Err(refused());
        }
        self.checked_bitcoin_psbt_for_recovery(
            &request.psbt,
            false,
            (after_attempt || request.input_preparation.is_some()).then_some(txid.as_str()),
        )
    }

    /// Persist one attempt before BDK and the exact verified original before ACK.
    /// A second call never signs, including after expiry, reload or file restore.
    pub async fn sign_bitcoin_recovery(
        &mut self,
        online: Online,
        request: BitcoinRecoveryRequest,
    ) -> Result<serde_json::Value, Error> {
        if request.input_preparation.is_some() {
            return Err(refused());
        }
        self.sign_bitcoin_self_payment(online, request).await
    }

    async fn sign_bitcoin_self_payment(
        &mut self,
        online: Online,
        request: BitcoinRecoveryRequest,
    ) -> Result<serde_json::Value, Error> {
        self._check_xprv()?;
        self.bitcoin_recovery_preflight(&request, false, super::safe_bitcoin::unix_seconds()?)?;
        self.require_wallet_rgb_regtest(&online).await?;
        self.sync_db_txos(false).await?;
        let psbt =
            self.bitcoin_recovery_preflight(&request, false, super::safe_bitcoin::unix_seconds()?)?;
        let txid = psbt.unsigned_tx.compute_txid().to_string();
        self.require_collaborative_original_unobserved(&txid)
            .await?;
        self.transfer_artifacts
            .entry(txid.clone())
            .or_default()
            .bitcoin_recovery = Some(BitcoinRecoveryRecord {
            request: request.clone(),
            attempted: true,
            broadcast_attempted: false,
            restored: false,
            cancelled: false,
        });
        self.flush().await?;
        let mut psbt =
            self.bitcoin_recovery_preflight(&request, true, super::safe_bitcoin::unix_seconds()?)?;
        self.require_collaborative_original_unobserved(&txid)
            .await?;
        self.bitcoin_recovery_preflight(&request, true, super::safe_bitcoin::unix_seconds()?)?;
        let signing = self._sign_psbt(&mut psbt, None);
        self.transfer_artifacts.get_mut(&txid).unwrap().signed_psbt = Some(psbt.to_string());
        self.flush().await?;
        signing?;
        verify_original(
            self.transfer_artifacts[&txid]
                .bitcoin_recovery
                .as_ref()
                .unwrap(),
            &psbt.to_string(),
        )?;
        if integer(&request.expires_at)? <= super::safe_bitcoin::unix_seconds()? {
            return Err(refused());
        }
        self.bitcoin_recovery_data(txid)
    }

    /// One explicit dispatch of the exact retained original. A timeout/error
    /// leaves the durable attempt set; the data API never retries dispatch.
    fn bitcoin_recovery_broadcast_preflight(
        &self,
        txid: &str,
        continuing: bool,
    ) -> Result<Psbt, Error> {
        self.bitcoin_recovery_data(txid.to_string())?;
        let artifact = self.transfer_artifacts.get(txid).ok_or_else(refused)?;
        let record = artifact.bitcoin_recovery.as_ref().ok_or_else(refused)?;
        if record.restored || record.cancelled || record.broadcast_attempted != continuing {
            return Err(refused());
        }
        let encoded = artifact.signed_psbt.as_deref().ok_or_else(refused)?;
        let signed = verify_original(record, encoded)?;
        self.checked_bitcoin_psbt_for_recovery(encoded, true, Some(txid))?;
        if record.request.input_preparation.is_some() {
            self.check_bitcoin_preparation_outputs(&record.request, &signed)?;
        } else if !matches!(
            self.bdk_wallet
                .derivation_of_spk(signed.unsigned_tx.output[0].script_pubkey.clone()),
            Some((KeychainKind::Internal, _))
        ) {
            return Err(refused());
        }
        Ok(signed)
    }

    /// Dispatch once after both host and Native attempts have committed.
    pub async fn broadcast_bitcoin_recovery(
        &mut self,
        online: Online,
        txid: String,
        expires_at: String,
    ) -> Result<String, Error> {
        let record = self
            .transfer_artifacts
            .get(&txid)
            .and_then(|artifact| artifact.bitcoin_recovery.as_ref())
            .ok_or_else(refused)?;
        if record.request.input_preparation.is_some() {
            return Err(refused());
        }
        self.broadcast_bitcoin_self_payment(online, txid, expires_at)
            .await
    }

    async fn broadcast_bitcoin_self_payment(
        &mut self,
        online: Online,
        txid: String,
        expires_at: String,
    ) -> Result<String, Error> {
        let expiry = integer(&expires_at)?;
        let check_deadline = || -> Result<(), Error> {
            let now = super::safe_bitcoin::unix_seconds()?;
            if expiry <= now || expiry > now.checked_add(120).ok_or_else(refused)? {
                return Err(refused());
            }
            Ok(())
        };
        check_deadline()?;
        self.bitcoin_recovery_data(txid.clone())?;
        self.require_wallet_rgb_regtest(&online).await?;
        self.sync_db_txos(false).await?;
        let signed = self.bitcoin_recovery_broadcast_preflight(&txid, false)?;
        self.require_collaborative_original_unobserved(&txid)
            .await?;
        check_deadline()?;
        self.transfer_artifacts
            .get_mut(&txid)
            .unwrap()
            .bitcoin_recovery
            .as_mut()
            .unwrap()
            .broadcast_attempted = true;
        self.flush().await?;
        check_deadline()?;
        if self.bitcoin_recovery_broadcast_preflight(&txid, true)? != signed {
            return Err(refused());
        }
        self.require_collaborative_original_unobserved(&txid)
            .await?;
        check_deadline()?;
        let tx = self
            ._broadcast_tx(signed.extract_tx().map_err(|_| refused())?)
            .await?;
        self.record_broadcast_transaction(tx, true).await?;
        self.update_backup_info(false)?;
        self.flush().await?;
        Ok(txid)
    }

    /// Read only the retained original. Expiry never grants another signature.
    pub fn bitcoin_recovery_data(&self, txid: String) -> Result<serde_json::Value, Error> {
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
        validate_bitcoin_recovery_artifact(&txid, artifact)?;
        let record = artifact.bitcoin_recovery.as_ref().ok_or_else(refused)?;
        retained_bitcoin_recovery_inputs(&txid, record)?;
        let complete = artifact
            .signed_psbt
            .as_deref()
            .is_some_and(|v| verify_original(record, v).is_ok());
        let mut result = serde_json::json!({"version":1,"kind":"wallet-bitcoin-recovery-original","scope":"RETAINED_BITCOIN_SELF_RECOVERY_NOT_SETTLEMENT","network":"regtest","txid":txid,"request":record.request,"attempted":record.attempted,"broadcast_attempted":record.broadcast_attempted,"restored":record.restored,"own_signatures_complete":complete,"signed_psbt":artifact.signed_psbt});
        if record.request.input_preparation.is_some() {
            result["kind"] = serde_json::json!("wallet-bitcoin-input-preparation");
            result["scope"] = serde_json::json!("OWN_INPUT_PREPARATION_NOT_MARKET_SETTLEMENT");
            result["cancelled"] = serde_json::json!(record.cancelled);
        }
        Ok(result)
    }
}

#[cfg(test)]
#[path = "bitcoin_recovery_tests.rs"]
pub(in crate::wallet) mod tests;
