//! Wallet-owned incoming originals. Retention grants no signature, credits no
//! balance and does not depend on the sender keeping a proxy online. The normal
//! incoming transfer state machine consumes these bytes only after confirmation.
use super::*;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ExternalReceiveOriginal {
    pub(crate) txid: String,
    pub(crate) contract_id: String,
    pub(crate) recipient_id: String,
    pub(crate) amount: String,
    pub(crate) vout: u32,
    pub(crate) batch_transfer_idx: i32,
    pub(crate) consignment_sha256: String,
    pub(crate) restored: bool,
}

fn refused() -> Error {
    Error::InvalidConsignment
}

/// Structural checks also run before snapshot installation. They establish
/// exact original identity, not fresh chain validity or current spendability.
pub(crate) fn validate_original(
    txid: &str,
    artifact: &TransferArtifacts,
    db: &DbData,
) -> Result<(), Error> {
    let original = artifact.external_receive.as_ref().ok_or_else(refused)?;
    let bytes = artifact
        .consignment_bytes
        .get(&original.contract_id)
        .ok_or_else(refused)?;
    let amount: u64 = original.amount.parse().map_err(|_| refused())?;
    if original.txid != txid
        || amount.to_string() != original.amount
        || !(1..=100_000).contains(&amount)
        || artifact.external_send.is_some()
        || artifact.consignment_bytes.len() != 1
        || bytes.is_empty()
        || bytes.len() > 2_097_152
        || <sha256::Hash as bitcoin::hashes::Hash>::hash(bytes).to_string()
            != original.consignment_sha256
    {
        return Err(refused());
    }
    let facts = super::super::rust_only::inspect_external_transfer_facts(
        bytes,
        txid.to_string(),
        original.contract_id.clone(),
        BitcoinNetwork::Regtest,
    )?;
    if facts["allocations"][original.vout.to_string()].as_str() != Some(original.amount.as_str()) {
        return Err(refused());
    }
    let transfer = RgbTransfer::load(bytes.as_slice()).map_err(|_| refused())?;
    if transfer.contract_id().to_string() != original.contract_id
        || AssetSchema::try_from(transfer.schema_id())? != AssetSchema::Nia
    {
        return Err(refused());
    }
    let witnesses: Vec<_> = transfer
        .bundles
        .iter()
        .filter(|b| b.pub_witness.txid().to_string() == txid)
        .collect();
    if witnesses.len() != 1 {
        return Err(refused());
    }
    let witness = witnesses[0].pub_witness.tx().ok_or_else(refused)?;
    let script =
        script_buf_from_recipient_id(original.recipient_id.clone())?.ok_or_else(refused)?;
    if witness.compute_txid().to_string() != txid
        || witness
            .output
            .get(original.vout as usize)
            .is_none_or(|o| o.script_pubkey != script)
    {
        return Err(refused());
    }
    let batch = db
        .batch_transfers
        .iter()
        .find(|b| b.idx == original.batch_transfer_idx)
        .ok_or_else(refused)?;
    let graph = batch.get_transfers(&db.asset_transfers, &db.transfers)?;
    if graph.asset_transfers_data.len() != 1
        || graph.asset_transfers_data[0].transfers.len() != 1
        || batch.txid.as_ref().is_some_and(|id| id != txid)
    {
        return Err(refused());
    }
    let group = &graph.asset_transfers_data[0];
    let receive = &group.transfers[0];
    if group.asset_transfer.asset_id.as_ref() != Some(&original.contract_id)
        || !receive.incoming
        || receive.recipient_id.as_ref() != Some(&original.recipient_id)
        || receive.requested_assignment != Some(Assignment::Fungible(amount))
        || !matches!(receive.recipient_type, Some(RecipientTypeFull::Witness { vout }) if vout.is_none() || vout == Some(original.vout))
    {
        return Err(refused());
    }
    Ok(())
}

pub(crate) fn validate_originals(
    artifacts: &HashMap<String, TransferArtifacts>,
    db: &DbData,
) -> Result<(), Error> {
    let mut recipients = HashSet::new();
    let mut batches = HashSet::new();
    for (txid, artifact) in artifacts {
        if let Some(original) = &artifact.external_receive {
            if !recipients.insert(&original.recipient_id)
                || !batches.insert(original.batch_transfer_idx)
            {
                return Err(refused());
            }
            validate_original(txid, artifact, db)?;
        }
    }
    Ok(())
}

impl Wallet {
    /// Validate standard RGB and live ancestry, bind an existing owned receive,
    /// then flush the exact original bytes before acknowledging inspection.
    pub async fn retain_external_rgb_receive(
        &mut self,
        bytes: &[u8],
        txid: String,
        contract: String,
        recipient: String,
        amount: String,
    ) -> Result<serde_json::Value, Error> {
        if self.bitcoin_network() != BitcoinNetwork::Regtest
            || self.online_data.is_none()
            || bytes.is_empty()
            || bytes.len() > 2_097_152
            || recipient.len() > 512
        {
            return Err(refused());
        }
        let units: u64 = amount.parse().map_err(|_| refused())?;
        if units.to_string() != amount || !(1..=100_000).contains(&units) {
            return Err(refused());
        }
        let facts = self.inspect_external_transfer(bytes, txid.clone(), contract.clone())?;
        self.validate_external_transfer_ancestry(bytes, txid.clone())
            .await?;
        let script = script_buf_from_recipient_id(recipient.clone())?.ok_or_else(refused)?;
        if !matches!(
            self.bdk_wallet.derivation_of_spk(script),
            Some((KeychainKind::External, _))
        ) {
            return Err(refused());
        }
        let data = self.database.get_db_data(false)?;
        let matches: Vec<_> = data
            .transfers
            .iter()
            .filter(|t| t.recipient_id.as_ref() == Some(&recipient))
            .collect();
        if matches.len() != 1 {
            return Err(refused());
        }
        let receive = matches[0];
        let asset = data
            .asset_transfers
            .iter()
            .find(|a| a.idx == receive.asset_transfer_idx)
            .ok_or_else(refused)?;
        let batch = data
            .batch_transfers
            .iter()
            .find(|b| b.idx == asset.batch_transfer_idx)
            .ok_or_else(refused)?;
        if !batch.waiting_counterparty()
            || batch.txid.is_some()
            || batch.expiration.is_none_or(|e| e <= now().unix_timestamp())
            || self.transfer_artifacts.iter().any(|(id, a)| {
                id != &txid
                    && a.external_receive.as_ref().is_some_and(|r| {
                        r.recipient_id == recipient || r.batch_transfer_idx == batch.idx
                    })
            })
        {
            return Err(refused());
        }
        let transfer = RgbTransfer::load(bytes).map_err(|_| refused())?;
        let script = script_buf_from_recipient_id(recipient.clone())?.ok_or_else(refused)?;
        let witnesses: Vec<_> = transfer
            .bundles
            .iter()
            .filter(|b| b.pub_witness.txid().to_string() == txid)
            .collect();
        if witnesses.len() != 1 {
            return Err(refused());
        }
        let witness = witnesses[0].pub_witness.tx().ok_or_else(refused)?;
        let outputs: Vec<_> = witness
            .output
            .iter()
            .enumerate()
            .filter(|(_, o)| o.script_pubkey == script)
            .collect();
        if outputs.len() != 1 {
            return Err(refused());
        }
        let vout = u32::try_from(outputs[0].0).map_err(|_| refused())?;
        if facts["allocations"][vout.to_string()].as_str() != Some(amount.as_str()) {
            return Err(refused());
        }
        let original = ExternalReceiveOriginal {
            txid: txid.clone(),
            contract_id: contract.clone(),
            recipient_id: recipient,
            amount,
            vout,
            batch_transfer_idx: batch.idx,
            consignment_sha256: <sha256::Hash as bitcoin::hashes::Hash>::hash(bytes).to_string(),
            restored: false,
        };
        let mut artifact = self
            .transfer_artifacts
            .get(&txid)
            .cloned()
            .unwrap_or_default();
        if let Some(old) = &artifact.external_receive {
            if old != &original {
                return Err(refused());
            }
        } else if artifact.partial_signing.is_some()
            || artifact.signed_psbt.is_some()
            || !artifact.consignment_bytes.is_empty()
            || artifact.external_send.is_some()
        {
            return Err(refused());
        }
        artifact.external_receive = Some(original);
        artifact.consignment_bytes.insert(contract, bytes.to_vec());
        validate_original(&txid, &artifact, &data)?;
        self.transfer_artifacts.insert(txid, artifact);
        self.flush().await?;
        Ok(facts)
    }

    /// Read only the original metadata. No consignment bytes, keys, signing,
    /// network request, ledger repair or new incoming intent is exposed.
    pub fn external_rgb_receive_data(&self, txid: String) -> Result<serde_json::Value, Error> {
        if self.bitcoin_network() != BitcoinNetwork::Regtest {
            return Err(refused());
        }
        let artifact = self.transfer_artifacts.get(&txid).ok_or_else(refused)?;
        validate_original(&txid, artifact, &self.database.get_db_data(false)?)?;
        serde_json::to_value(artifact.external_receive.as_ref().unwrap())
            .map_err(InternalError::from)
            .map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_and_non_regtest_retention_never_mutates_or_suspends() {
        let mut wallet = super::super::safe_bitcoin::tests::wallet();
        for network in [BitcoinNetwork::Regtest, BitcoinNetwork::Mainnet] {
            wallet.wallet_data.bitcoin_network = network;
            let before = serde_json::to_value(&wallet.transfer_artifacts).unwrap();
            {
                let mut context = std::task::Context::from_waker(std::task::Waker::noop());
                let mut future = std::pin::pin!(wallet.retain_external_rgb_receive(
                    b"unfunded-invalid-proof",
                    "11".repeat(32),
                    "contract".into(),
                    "recipient".into(),
                    "1".into()
                ));
                assert!(matches!(
                    std::future::Future::poll(future.as_mut(), &mut context),
                    std::task::Poll::Ready(Err(Error::InvalidConsignment))
                ));
            }
            assert_eq!(
                serde_json::to_value(&wallet.transfer_artifacts).unwrap(),
                before
            );
            assert!(wallet.received_consignments.is_empty());
            assert!(
                wallet
                    .database
                    .get_db_data(false)
                    .unwrap()
                    .transfers
                    .is_empty()
            );
        }
    }

    #[test]
    fn failed_receive_history_is_preserved_in_single_and_bulk_delete() {
        // Refusal-journal fixture, not an accepted standard RGB proof.
        let mut wallet = super::super::safe_bitcoin::tests::wallet();
        let idx = wallet
            .database
            .set_batch_transfer(DbBatchTransferActMod {
                status: ActiveValue::Set(TransferStatus::Failed),
                ..Default::default()
            })
            .unwrap();
        wallet.transfer_artifacts.insert(
            "original".into(),
            TransferArtifacts {
                external_receive: Some(ExternalReceiveOriginal {
                    txid: "original".into(),
                    contract_id: "contract".into(),
                    recipient_id: "recipient".into(),
                    amount: "1".into(),
                    vout: 2,
                    batch_transfer_idx: idx,
                    consignment_sha256: "digest".into(),
                    restored: false,
                }),
                ..Default::default()
            },
        );
        let before = serde_json::to_value(wallet.database.as_ref()).unwrap();
        assert!(matches!(
            wallet.delete_transfers(Some(idx), false),
            Err(Error::CannotDeleteBatchTransfer)
        ));
        assert!(!wallet.delete_transfers(None, false).unwrap());
        assert_eq!(
            serde_json::to_value(wallet.database.as_ref()).unwrap(),
            before
        );
        assert!(wallet.transfer_artifacts.contains_key("original"));
    }

    #[test]
    fn matching_metadata_or_digest_never_substitutes_for_standard_rgb_proof() {
        let bytes = b"unfunded-invalid-proof".to_vec();
        let txid = "11".repeat(32);
        let artifact = TransferArtifacts {
            external_receive: Some(ExternalReceiveOriginal {
                txid: txid.clone(),
                contract_id: "contract".into(),
                recipient_id: "recipient".into(),
                amount: "1".into(),
                vout: 2,
                batch_transfer_idx: 1,
                consignment_sha256: <sha256::Hash as bitcoin::hashes::Hash>::hash(&bytes)
                    .to_string(),
                restored: false,
            }),
            consignment_bytes: HashMap::from([("contract".into(), bytes)]),
            ..Default::default()
        };
        let db = crate::database::memory_db::InMemoryDb::new()
            .get_db_data(false)
            .unwrap();
        assert!(validate_original(&txid, &artifact, &db).is_err());
        assert!(validate_originals(&HashMap::from([(txid, artifact)]), &db).is_err());
    }
}
