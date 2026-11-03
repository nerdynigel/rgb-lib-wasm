//! Read-only chain reconciliation of a retained incoming witness transfer.
//! A timeout, missing transaction or mempool replacement never proves failure.
use super::*;

/// Retained proof that a different confirmed transaction spent an incoming
/// proposal's input. No Stock acceptance or outgoing reservation release occurs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IncomingRgbConflict {
    /// Receipt format, currently 1.
    pub version: u8,
    /// This recovery mechanism is restricted to regtest.
    pub network: String,
    /// Original incoming witness transaction.
    pub original_txid: String,
    /// Independently observed competing transaction.
    pub conflicting_txid: String,
    /// Exact independently fetched competing transaction, including witness.
    pub conflicting_transaction_hex: String,
    /// Authoritative RGB contract identity.
    pub contract_id: String,
    /// Original wallet-owned witness recipient.
    pub recipient_id: String,
    /// Original incoming batch, retained in the real database.
    pub batch_transfer_idx: i32,
    /// SHA-256 of the retained original standard consignment.
    pub consignment_sha256: String,
    /// Observed nonzero confirmed block height.
    pub confirmed_height: u32,
    /// Independently observed confirmations, at least one and the requested minimum.
    pub confirmations: u64,
}

fn original_witness(bytes: &[u8], txid: &str) -> Result<BdkTransaction, Error> {
    if bytes.len() > 2_097_152 {
        return Err(Error::InvalidConsignment);
    }
    let consignment = RgbTransfer::load(bytes).map_err(InternalError::from)?;
    let mut witnesses = consignment
        .bundles
        .iter()
        .filter(|b| b.pub_witness.txid().to_string() == txid);
    let original = witnesses
        .next()
        .and_then(|b| b.pub_witness.tx())
        .cloned()
        .ok_or(Error::InvalidConsignment)?;
    if witnesses.next().is_some()
        || original.compute_txid().to_string() != txid
        || original.input.is_empty()
        || original.input.len() > 256
    {
        return Err(Error::InvalidConsignment);
    }
    Ok(original)
}

/// Structural verification also runs on snapshot/backup restore. This does not
/// claim a fresh chain observation: the receipt preserves what was verified at
/// classification, and never grants a new signature or spends an input.
pub(crate) fn validate_retained_conflicts(
    receipts: &HashMap<String, IncomingRgbConflict>,
    consignments: &HashMap<String, Vec<u8>>,
    db: &DbData,
) -> Result<(), Error> {
    for (recipient, receipt) in receipts {
        let batch = db
            .batch_transfers
            .iter()
            .find(|b| b.idx == receipt.batch_transfer_idx)
            .ok_or(Error::InvalidConsignment)?;
        let graph = batch.get_transfers(&db.asset_transfers, &db.transfers)?;
        if receipt.version != 1
            || receipt.network != "regtest"
            || recipient != &receipt.recipient_id
            || receipt.confirmed_height == 0
            || receipt.confirmations < u64::from(batch.min_confirmations.max(1))
            || !batch.status.failed()
            || batch.txid.as_ref() != Some(&receipt.original_txid)
            || graph.asset_transfers_data.len() != 1
            || graph.asset_transfers_data[0].transfers.len() != 1
        {
            return Err(Error::InvalidConsignment);
        }
        let group = &graph.asset_transfers_data[0];
        let transfer = &group.transfers[0];
        let Some(RecipientTypeFull::Witness { vout: Some(vout) }) = transfer.recipient_type else {
            return Err(Error::InvalidConsignment);
        };
        if !transfer.incoming
            || transfer.recipient_id.as_ref() != Some(recipient)
            || group.asset_transfer.asset_id.as_ref() != Some(&receipt.contract_id)
        {
            return Err(Error::InvalidConsignment);
        }
        let bytes = consignments
            .get(recipient)
            .ok_or(Error::InvalidConsignment)?;
        if <sha256::Hash as bitcoin::hashes::Hash>::hash(bytes).to_string()
            != receipt.consignment_sha256
        {
            return Err(Error::InvalidConsignment);
        }
        let original = original_witness(bytes, &receipt.original_txid)?;
        let consignment = RgbTransfer::load(bytes.as_slice()).map_err(InternalError::from)?;
        let script =
            script_buf_from_recipient_id(recipient.clone())?.ok_or(Error::InvalidConsignment)?;
        if consignment.contract_id().to_string() != receipt.contract_id
            || AssetSchema::try_from(consignment.schema_id())? != AssetSchema::Nia
            || original
                .output
                .get(vout as usize)
                .is_none_or(|o| o.script_pubkey != script)
            || receipt.conflicting_transaction_hex.len() > 8_000_000
        {
            return Err(Error::InvalidConsignment);
        }
        let conflicting: BdkTransaction = bitcoin::consensus::deserialize(
            &hex::decode(&receipt.conflicting_transaction_hex)
                .map_err(|_| Error::InvalidConsignment)?,
        )
        .map_err(|_| Error::InvalidConsignment)?;
        if hex::encode(bitcoin::consensus::serialize(&conflicting))
            != receipt.conflicting_transaction_hex
            || conflicting.compute_txid().to_string() != receipt.conflicting_txid
            || !distinct_input_conflict(&original, &conflicting)
        {
            return Err(Error::InvalidConsignment);
        }
    }
    Ok(())
}

fn confirmed_input_conflict(
    original: &BdkTransaction,
    conflicting: &BdkTransaction,
    expected: Txid,
    height: u32,
    confirmations: u64,
    minimum: u8,
    input: bitcoin::OutPoint,
) -> bool {
    height > 0
        && confirmations >= u64::from(minimum.max(1))
        && conflicting.compute_txid() == expected
        && distinct_input_conflict(original, conflicting)
        && original.input.iter().any(|i| i.previous_output == input)
        && conflicting.input.iter().any(|i| i.previous_output == input)
}

impl Wallet {
    /// Existing refresh calls this only for an incoming witness whose original
    /// transaction is absent. Outspend hints select candidates; independently
    /// fetched transaction bytes and confirmations supply the actual proof.
    pub(super) async fn reconcile_incoming_witness_conflict(
        &mut self,
        batch: &DbBatchTransfer,
        db: &DbData,
    ) -> Result<Option<DbBatchTransfer>, Error> {
        if self.bitcoin_network() != BitcoinNetwork::Regtest
            || !batch.status.waiting_confirmations()
            || !batch.incoming(&db.asset_transfers, &db.transfers)?
        {
            return Ok(None);
        }
        let graph = batch.get_transfers(&db.asset_transfers, &db.transfers)?;
        if graph.asset_transfers_data.len() != 1
            || graph.asset_transfers_data[0].transfers.len() != 1
        {
            return Ok(None);
        }
        let (asset, transfer) = self.database.get_incoming_transfer(&graph)?;
        let Some(RecipientTypeFull::Witness { vout: Some(vout) }) = transfer.recipient_type else {
            return Ok(None);
        };
        let txid = batch.txid.as_ref().ok_or(Error::InvalidConsignment)?;
        let contract = asset.asset_id.as_ref().ok_or(Error::InvalidConsignment)?;
        let recipient = transfer
            .recipient_id
            .as_ref()
            .ok_or(Error::InvalidConsignment)?;
        let bytes = self
            .received_consignments
            .get(recipient)
            .ok_or(Error::InvalidConsignment)?
            .clone();
        let original = original_witness(&bytes, txid)?;
        let script =
            script_buf_from_recipient_id(recipient.clone())?.ok_or(Error::InvalidConsignment)?;
        if original
            .output
            .get(vout as usize)
            .is_none_or(|o| o.script_pubkey != script)
            || !matches!(
                self.bdk_wallet.derivation_of_spk(script),
                Some((KeychainKind::External, _))
            )
        {
            return Err(Error::InvalidConsignment);
        }
        // Standard RGB validation of this contract and allocation, followed by
        // live validation of every historical witness. The original alone is
        // tentative; no private Stock reconstruction or balance mutation.
        let inspected = self.inspect_external_transfer(&bytes, txid.clone(), contract.clone())?;
        if inspected["allocations"][vout.to_string()]
            .as_str()
            .and_then(|v| v.parse::<u64>().ok())
            .is_none_or(|v| v == 0)
        {
            return Err(Error::InvalidConsignment);
        }
        self.validate_external_transfer_ancestry(&bytes, txid.clone())
            .await?;
        let genesis = bitcoin::blockdata::constants::genesis_block(BdkNetwork::Regtest)
            .block_hash()
            .to_string();
        if self.indexer().block_hash(0).await? != genesis {
            return Err(Error::InvalidConsignment);
        }
        for input in &original.input {
            let hint = match self.indexer() {
                Indexer::EsploraAsync(client) => client
                    .get_output_status(
                        &input.previous_output.txid,
                        u64::from(input.previous_output.vout),
                    )
                    .await
                    .map_err(IndexerError::from)?,
            };
            let Some(hint) = hint.filter(|h| h.spent) else {
                continue;
            };
            let Some(conflict_id) = hint.txid.filter(|id| id.to_string() != *txid) else {
                continue;
            };
            let confirmations = self
                .indexer()
                .get_tx_confirmations(&conflict_id.to_string())
                .await?
                .unwrap_or(0);
            if confirmations < u64::from(batch.min_confirmations.max(1)) {
                continue;
            }
            let Some((conflicting, Some(height), _)) =
                self.indexer().get_tx_with_status(&conflict_id).await?
            else {
                continue;
            };
            if !confirmed_input_conflict(
                &original,
                &conflicting,
                conflict_id,
                height,
                confirmations,
                batch.min_confirmations,
                input.previous_output,
            ) {
                return Err(Error::InvalidConsignment);
            }
            // Recheck the original immediately before classification. An
            // observed original, including an unconfirmed one, stays pending.
            if self.indexer().get_tx_confirmations(txid).await?.is_some() {
                return Ok(None);
            }
            let receipt = IncomingRgbConflict {
                version: 1,
                network: "regtest".into(),
                original_txid: txid.clone(),
                conflicting_txid: conflict_id.to_string(),
                conflicting_transaction_hex: hex::encode(bitcoin::consensus::serialize(
                    &conflicting,
                )),
                contract_id: contract.clone(),
                recipient_id: recipient.clone(),
                batch_transfer_idx: batch.idx,
                consignment_sha256: <sha256::Hash as bitcoin::hashes::Hash>::hash(&bytes)
                    .to_string(),
                confirmed_height: height,
                confirmations,
            };
            if self
                .incoming_conflicts
                .get(recipient)
                .is_some_and(|old| old != &receipt)
            {
                return Err(Error::InvalidConsignment);
            }
            let mut failed: DbBatchTransferActMod = batch.clone().into();
            failed.status = ActiveValue::Set(TransferStatus::Failed);
            let updated = self.database.update_batch_transfer(&mut failed)?;
            self.incoming_conflicts.insert(recipient.clone(), receipt);
            // Keep the consignment, request, colorings and exact chain receipt.
            // Only this incoming batch changes; Stock and outgoing holds do not.
            self.flush().await?;
            return Ok(Some(updated));
        }
        Ok(None)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    pub(crate) fn receipt() -> IncomingRgbConflict {
        IncomingRgbConflict {
            version: 1,
            network: "regtest".into(),
            original_txid: "11".repeat(32),
            conflicting_txid: "22".repeat(32),
            conflicting_transaction_hex: "00".into(),
            contract_id: "test-contract".into(),
            recipient_id: "test-recipient".into(),
            batch_transfer_idx: 1,
            consignment_sha256: "33".repeat(32),
            confirmed_height: 183,
            confirmations: 2,
        }
    }
    #[test]
    fn confirmation_absence_unrelated_input_and_same_txid_never_classify_failure() {
        let input = bitcoin::OutPoint {
            txid: "11".repeat(32).parse().unwrap(),
            vout: 1,
        };
        let original = BdkTransaction {
            version: bitcoin::transaction::Version::TWO,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: vec![bitcoin::TxIn {
                previous_output: input,
                script_sig: bitcoin::ScriptBuf::new(),
                sequence: bitcoin::Sequence::MAX,
                witness: bitcoin::Witness::new(),
            }],
            output: vec![bitcoin::TxOut {
                value: bitcoin::Amount::from_sat(1000),
                script_pubkey: bitcoin::ScriptBuf::new(),
            }],
        };
        let mut conflict = original.clone();
        conflict.output[0].value = bitcoin::Amount::from_sat(900);
        let id = conflict.compute_txid();
        assert!(confirmed_input_conflict(
            &original, &conflict, id, 183, 2, 2, input
        ));
        assert!(!confirmed_input_conflict(
            &original, &conflict, id, 183, 0, 0, input
        ));
        assert!(!confirmed_input_conflict(
            &original, &conflict, id, 0, 2, 1, input
        ));
        assert!(!confirmed_input_conflict(
            &original, &conflict, id, 183, 1, 2, input
        ));
        assert!(!confirmed_input_conflict(
            &original,
            &conflict,
            original.compute_txid(),
            183,
            2,
            1,
            input
        ));
        assert!(!confirmed_input_conflict(
            &original,
            &conflict,
            id,
            183,
            2,
            1,
            bitcoin::OutPoint::null()
        ));
        conflict.input[0].previous_output.vout += 1;
        assert!(!confirmed_input_conflict(
            &original,
            &conflict,
            conflict.compute_txid(),
            183,
            2,
            1,
            input
        ));
        let mut witness_variant = original.clone();
        witness_variant.input[0].witness.push([1, 2, 3]);
        assert!(!confirmed_input_conflict(
            &original,
            &witness_variant,
            original.compute_txid(),
            183,
            2,
            1,
            input
        ));
    }

    #[test]
    fn malformed_retained_receipt_cannot_replace_wallet_state() {
        let mut wallet = super::super::safe_bitcoin::tests::wallet();
        let mut snapshot: super::super::super::idb_store::WalletSnapshot =
            serde_json::from_value(serde_json::json!({
                "sequence": 1, "db": wallet.database.as_ref(), "bdk_changeset": null,
                "transfer_artifacts": {}, "received_consignments": {}, "incoming_conflicts": {}
            }))
            .unwrap();
        snapshot
            .incoming_conflicts
            .insert("test-recipient".into(), receipt());
        let before = serde_json::to_vec(wallet.database.as_ref()).unwrap();
        assert!(wallet.restore_from_snapshot(snapshot).is_err());
        assert_eq!(
            serde_json::to_vec(wallet.database.as_ref()).unwrap(),
            before
        );
        assert!(wallet.incoming_conflicts.is_empty());
    }
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn absent_original_uses_raw_404_without_requesting_status_or_changing_state() {
        use std::{
            io::{Read, Write},
            net::TcpListener,
        };
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let id = "11".repeat(32);
        let expected = format!("GET /tx/{id}/raw HTTP/1.1");
        let server = std::thread::spawn(move || {
            listener.set_nonblocking(true).unwrap();
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
            loop {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        stream
                            .set_read_timeout(Some(std::time::Duration::from_secs(3)))
                            .unwrap();
                        let mut bytes = [0; 4096];
                        let n = stream.read(&mut bytes).unwrap();
                        assert!(
                            std::str::from_utf8(&bytes[..n])
                                .unwrap()
                                .starts_with(&expected)
                        );
                        stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();
                        return;
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(std::time::Instant::now() < deadline);
                        std::thread::sleep(std::time::Duration::from_millis(5));
                    }
                    Err(e) => panic!("loopback HTTP fixture failed: {e}"),
                }
            }
        });
        let indexer = crate::utils::build_indexer(&url).unwrap();
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        assert_eq!(
            rt.block_on(indexer.get_tx_confirmations(&id)).unwrap(),
            None
        );
        assert!(
            rt.block_on(indexer.get_tx_confirmations("malformed"))
                .is_err()
        );
        server.join().unwrap();
    }
}
