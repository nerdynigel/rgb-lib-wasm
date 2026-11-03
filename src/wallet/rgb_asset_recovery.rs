//! A new, explicitly approved standard RGB self-transfer which conflicts with
//! one expired ordinary original. Originals, reservations and Stock are retained
//! until independently confirmed Bitcoin conflict reconciliation. Never replay.
use super::*;

/// Immutable purpose of a separately approved wallet-owned RGB recovery.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WalletRgbRecoveryBinding {
    /// Explicit capability version; currently one.
    pub version: u8,
    /// Exact existing ordinary operation; different from the new operation.
    pub original_operation_id: String,
    /// Exact retained original transaction.
    pub original_txid: String,
    /// SHA256 of the complete original signed Bitcoin transaction bytes.
    pub original_signed_transaction_sha256: String,
    /// Approved maximum miner fee for the new recovery, in whole sats.
    #[serde(with = "integer_string")]
    pub max_fee_sats: u64,
}

fn current_seconds() -> Result<u64, Error> {
    #[cfg(target_arch = "wasm32")]
    let n = now().unix_timestamp();
    #[cfg(not(target_arch = "wasm32"))]
    let n = time::OffsetDateTime::now_utc().unix_timestamp();
    u64::try_from(n).map_err(|_| invalid())
}

impl WalletRgbRecoveryBinding {
    pub(super) fn validate(&self, new_operation: &str) -> Result<(), Error> {
        let hex32 = |v: &str| {
            v.len() == 64
                && v.bytes()
                    .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
        };
        if self.version != 1
            || !valid_operation_id(&self.original_operation_id)
            || self.original_operation_id == new_operation
            || !hex32(&self.original_txid)
            || !hex32(&self.original_signed_transaction_sha256)
            || !(1..=10_000).contains(&self.max_fee_sats)
        {
            return Err(invalid());
        }
        Ok(())
    }
}

impl Wallet {
    fn rgb_recovery_original(
        &self,
        binding: &WalletRgbRecoveryBinding,
    ) -> Result<(&str, &TransferArtifacts, WalletRgbSendRequest, Psbt), Error> {
        let (id, art) = self
            .wallet_transfer_by_operation(&binding.original_operation_id)?
            .ok_or_else(invalid)?;
        let ext = art.external_send.as_ref().ok_or_else(invalid)?;
        let original_binding = ext.wallet_transfer.as_ref().ok_or_else(invalid)?;
        let mut request: WalletRgbSendRequest =
            serde_json::from_str(&original_binding.request_json).map_err(|_| invalid())?;
        let canonical = validate_request(&mut request)?;
        let signed = Psbt::from_str(art.signed_psbt.as_ref().ok_or_else(invalid)?)?;
        let tx = signed.clone().extract_tx().map_err(|_| invalid())?;
        if request.recovery_of.is_some()
            || request.operation_id != binding.original_operation_id
            || canonical.binding.request_json != original_binding.request_json
            || id != &binding.original_txid
            || tx.compute_txid().to_string() != *id
            || Self::tx_stripped_hex(&tx) != ext.expected_tx_hex
            || <sha256::Hash as bitcoin::hashes::Hash>::hash(&bitcoin::consensus::serialize(&tx))
                .to_string()
                != binding.original_signed_transaction_sha256
            || !original_binding.signing_attempted
            || tx.input.is_empty()
            || tx.input.len() > 128
            || tx.input.iter().any(|i| {
                i.witness.len() != 1 || i.witness.iter().next().is_none_or(|s| s.len() != 64)
            })
        {
            return Err(invalid());
        }
        Ok((id, art, request, signed))
    }

    pub(super) fn checked_wallet_rgb_recovery_source(
        &self,
        request: &WalletRgbSendRequest,
    ) -> Result<Option<String>, Error> {
        let Some(binding) = &request.recovery_of else {
            return Ok(None);
        };
        binding.validate(&request.operation_id)?;
        let (id, art, original, psbt) = self.rgb_recovery_original(binding)?;
        let external = art.external_send.as_ref().ok_or_else(invalid)?;
        let retained = external.wallet_transfer.as_ref().ok_or_else(invalid)?;
        let batch = self.batch_transfer_by_txid(id)?.ok_or_else(invalid)?;
        if self.bitcoin_network() != BitcoinNetwork::Regtest
            || external.status != ExternalSendStatus::Prepared
            || retained.restored
            || batch.status != TransferStatus::Initiated
            || original.expires_at > current_seconds()?
            || request.asset_id != original.asset_id
            || request.min_confirmations != original.min_confirmations
        {
            return Err(invalid());
        }
        self.assert_wallet_rgb_stock_inputs(id, &psbt)?;
        let units = self.wallet_rgb_original_input_units(id)?;
        let total = units
            .values()
            .try_fold(0u64, |n, a| n.checked_add(*a).ok_or_else(invalid))?;
        if request.amount != total {
            return Err(invalid());
        }
        let beneficiary =
            XChainNet::<Beneficiary>::from_str(&request.recipient_id).map_err(|_| invalid())?;
        let Beneficiary::WitnessVout(pay_to, None) = beneficiary.into_inner() else {
            return Err(invalid());
        };
        let script = pay_to.to_script();
        if !matches!(
            self.bdk_wallet.derivation_of_spk(script.clone()),
            Some((KeychainKind::External, _))
        ) {
            return Err(invalid());
        }
        let db = self.database.get_db_data(false)?;
        let incoming: Vec<_> = db
            .transfers
            .iter()
            .filter(|t| {
                t.incoming && t.recipient_id.as_deref() == Some(request.recipient_id.as_str())
            })
            .collect();
        if incoming.len() != 1
            || incoming[0].requested_assignment != Some(Assignment::Fungible(total))
        {
            return Err(invalid());
        }
        let incoming_asset = db
            .asset_transfers
            .iter()
            .find(|a| a.idx == incoming[0].asset_transfer_idx)
            .ok_or_else(invalid)?;
        let incoming_batch = db
            .batch_transfers
            .iter()
            .find(|b| b.idx == incoming_asset.batch_transfer_idx)
            .ok_or_else(invalid)?;
        let new_txid = self
            .wallet_transfer_by_operation(&request.operation_id)?
            .map(|(id, _)| id.as_str());
        if incoming_asset.asset_id.as_deref() != Some(request.asset_id.as_str())
            || incoming_batch.status.failed()
            || incoming_batch.min_confirmations != request.min_confirmations
            || incoming_batch
                .txid
                .as_deref()
                .is_some_and(|id| Some(id) != new_txid)
            || (incoming_batch.txid.is_none()
                && !self
                    .database
                    .iter_pending_witness_scripts()?
                    .iter()
                    .any(|r| r.script == script.to_hex_string()))
        {
            return Err(invalid());
        }
        // One retained recovery per original. A second name cannot bypass an
        // attempted/unknown recovery. Only proven unsigned cancellation permits
        // a replacement; the cancelled original remains retained.
        for (other_id, other) in &self.transfer_artifacts {
            let Some(ext) = &other.external_send else {
                continue;
            };
            let Some(b) = &ext.wallet_transfer else {
                continue;
            };
            let r: WalletRgbSendRequest =
                serde_json::from_str(&b.request_json).map_err(|_| invalid())?;
            if r.operation_id == request.operation_id
                || r.recovery_of
                    .as_ref()
                    .is_none_or(|v| v.original_txid != *id)
            {
                continue;
            }
            if ext.status != ExternalSendStatus::Cancelled
                || b.restored
                || b.signing_attempted
                || b.broadcast_attempted
                || other.signed_psbt.is_some()
                || !self
                    .batch_transfer_by_txid(other_id)?
                    .ok_or_else(invalid)?
                    .status
                    .failed()
            {
                return Err(Error::ExternalSendConflict {
                    txid: other_id.clone(),
                });
            }
        }
        Ok(Some(id.to_string()))
    }

    pub(super) fn wallet_rgb_recovery_source_for_tx(
        &self,
        txid: &str,
    ) -> Result<Option<String>, Error> {
        let Some(b) = self
            .transfer_artifacts
            .get(txid)
            .and_then(|a| a.external_send.as_ref())
            .and_then(|e| e.wallet_transfer.as_ref())
        else {
            return Ok(None);
        };
        let request: WalletRgbSendRequest =
            serde_json::from_str(&b.request_json).map_err(|_| invalid())?;
        self.checked_wallet_rgb_recovery_source(&request)
    }

    /// View only: omit reservations of the exact unconsumed source proposal.
    /// No database row, Stock state, original artifact or input hold is changed.
    pub(in crate::wallet::online) fn wallet_rgb_recovery_db_view(
        &self,
        mut db: DbData,
        request: &WalletRgbSendRequest,
    ) -> Result<DbData, Error> {
        if let Some(id) = self.checked_wallet_rgb_recovery_source(request)? {
            let batch = self.batch_transfer_by_txid(&id)?.ok_or_else(invalid)?;
            let assets: HashSet<_> = db
                .asset_transfers
                .iter()
                .filter(|a| a.batch_transfer_idx == batch.idx)
                .map(|a| a.idx)
                .collect();
            db.colorings
                .retain(|c| !assets.contains(&c.asset_transfer_idx));
        }
        Ok(db)
    }

    pub(super) fn wallet_rgb_recovery_candidate_inputs(
        &self,
        original: &str,
        request: &WalletRgbSendRequest,
        excluded: &mut HashSet<OutPoint>,
    ) -> Result<Vec<LocalUnspent>, Error> {
        let units = self.wallet_rgb_original_input_units(original)?;
        let db = self.wallet_rgb_recovery_db_view(self.database.get_db_data(false)?, request)?;
        let unspent = self.database.get_unspent_txos(db.txos.clone())?;
        let mut candidates = self.database.get_rgb_allocations(
            unspent,
            Some(db.colorings),
            Some(db.batch_transfers),
            Some(db.asset_transfers),
            Some(db.transfers),
        )?;
        candidates.retain(|u| units.contains_key(&u.utxo.outpoint().to_string()));
        let candidates = self.wallet_rgb_candidate_inputs_except(
            candidates,
            &request.asset_id,
            excluded,
            Some(original),
        )?;
        let mut actual = BTreeMap::new();
        for u in &candidates {
            let n = u.rgb_allocations.iter().try_fold(0u64, |n, a| {
                let Assignment::Fungible(v) = a.assignment else {
                    return Err(invalid());
                };
                n.checked_add(v).ok_or_else(invalid)
            })?;
            actual.insert(u.utxo.outpoint().to_string(), n);
        }
        if actual != units {
            return Err(invalid());
        }
        Ok(candidates)
    }

    pub(super) fn check_wallet_rgb_recovery_transaction(
        &self,
        request: &WalletRgbSendRequest,
        psbt: &Psbt,
    ) -> Result<(), Error> {
        let Some(binding) = &request.recovery_of else {
            return Ok(());
        };
        let original = self
            .checked_wallet_rgb_recovery_source(request)?
            .ok_or_else(invalid)?;
        let expected = self.wallet_rgb_original_input_units(&original)?;
        self.assert_wallet_rgb_stock_input_units(&request.asset_id, psbt, &expected)?;
        let coins = self
            .bdk_wallet
            .list_unspent()
            .take(4097)
            .collect::<Vec<_>>();
        if coins.len() > 4096 || psbt.inputs.len() != psbt.unsigned_tx.input.len() {
            return Err(invalid());
        }
        let fees = if self
            .wallet_transfer_by_operation(&request.operation_id)?
            .is_some()
        {
            self.bitcoin_rgb_fee_inputs(&request.operation_id)?
        } else {
            self.bitcoin_spend_view()?.inputs
        };
        for (input, details) in psbt.unsigned_tx.input.iter().zip(&psbt.inputs) {
            let coin = coins
                .iter()
                .find(|c| c.outpoint == input.previous_output)
                .ok_or_else(invalid)?;
            if !matches!(
                coin.chain_position,
                bdk_wallet::chain::ChainPosition::Confirmed { .. }
            ) || coin.is_spent
                || details.witness_utxo.as_ref() != Some(&coin.txout)
                || (!expected.contains_key(&input.previous_output.to_string())
                    && !fees
                        .iter()
                        .any(|f| f.outpoint == input.previous_output.to_string()))
            {
                return Err(invalid());
            }
        }
        let inputs: HashSet<_> = psbt
            .unsigned_tx
            .input
            .iter()
            .map(|i| i.previous_output.to_string())
            .collect();
        let input_sats = psbt.inputs.iter().try_fold(0u64, |n, i| {
            n.checked_add(i.witness_utxo.as_ref().ok_or_else(invalid)?.value.to_sat())
                .ok_or_else(invalid)
        })?;
        let output_sats = psbt.unsigned_tx.output.iter().try_fold(0u64, |n, o| {
            n.checked_add(o.value.to_sat()).ok_or_else(invalid)
        })?;
        let fee = input_sats.checked_sub(output_sats).ok_or_else(invalid)?;
        if psbt.unsigned_tx.compute_txid().to_string() == original
            || !expected.keys().all(|p| inputs.contains(p))
            || fee == 0
            || fee > binding.max_fee_sats
            || psbt.unsigned_tx.output.iter().any(|o| {
                if o.script_pubkey.is_op_return() {
                    o.value.to_sat() != 0
                } else {
                    !self.bdk_wallet.is_mine(o.script_pubkey.clone())
                }
            })
        {
            return Err(invalid());
        }
        Ok(())
    }

    pub(in crate::wallet::online) async fn assert_wallet_rgb_recovery_original_unobserved(
        &self,
        txid: &str,
    ) -> Result<(), Error> {
        let Some(original) = self.wallet_rgb_recovery_source_for_tx(txid)? else {
            return Ok(());
        };
        if self
            .indexer()
            .get_tx_with_status(&Txid::from_str(&original).map_err(|_| invalid())?)
            .await?
            .is_some()
        {
            return Err(Error::ExternalSendConflict { txid: original });
        }
        Ok(())
    }

    pub(super) async fn observe_wallet_rgb_recovery_conflict(
        &mut self,
        txid: &str,
        observed: &BdkTransaction,
    ) -> Result<bool, Error> {
        let binding = self
            .transfer_artifacts
            .get(txid)
            .and_then(|a| a.external_send.as_ref())
            .and_then(|e| e.wallet_transfer.as_ref())
            .ok_or_else(invalid)?;
        let request: WalletRgbSendRequest =
            serde_json::from_str(&binding.request_json).map_err(|_| invalid())?;
        let Some(recovery) = request.recovery_of else {
            return Ok(true);
        };
        recovery.validate(&request.operation_id)?;
        let (original, _, _, _) = self.rgb_recovery_original(&recovery)?;
        let original = original.to_string();
        // Observe-only recovery also works after a complete pending-file restore.
        // Never consume Stock or release the old proposal on a mempool response.
        if self
            .indexer()
            .get_tx_confirmations(txid)
            .await?
            .unwrap_or(0)
            < u64::from(request.min_confirmations)
        {
            return Ok(false);
        }
        self.reconcile_external_rgb_conflict(original, observed)
            .await?;
        Ok(true)
    }

    /// Explicit new capability. Never admits a recovery marker through the
    /// ordinary preparation API or mutates the old request/approval deadline.
    pub async fn prepare_wallet_rgb_recovery(
        &mut self,
        online: Online,
        request: WalletRgbSendRequest,
    ) -> Result<ExternalSendPrepareResult, Error> {
        self.require_wallet_rgb_regtest(&online).await?;
        let original = self
            .checked_wallet_rgb_recovery_source(&request)?
            .ok_or_else(invalid)?;
        // Preserve exact idempotent reads of an existing recovery even after
        // its review expires. Never choose new inputs under the same identity.
        if self
            .wallet_transfer_by_operation(&request.operation_id)?
            .is_none()
        {
            let now = current_seconds()?;
            if request.expires_at <= now
                || request.expires_at > now.checked_add(120).ok_or_else(invalid)?
            {
                return Err(invalid());
            }
        }
        if self
            .indexer()
            .get_tx_with_status(&Txid::from_str(&original).map_err(|_| invalid())?)
            .await?
            .is_some()
        {
            return Err(Error::ExternalSendConflict { txid: original });
        }
        self.prepare_wallet_rgb_send_inner(online, request).await
    }
}

#[cfg(test)]
#[path = "rgb_asset_recovery_tests.rs"]
mod tests;
