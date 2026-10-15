//! Frozen, read-only chain/history validation for an exact incoming NIA receipt.
//! Indexer observations are evidence, never consent or a consuming capability.
use super::*;
use crate::utils::PreFetchResolver;
use bdk_wallet::chain::{BlockId, ConfirmationBlockTime};
use esplora_client::TxStatus;

const MAX_HISTORY_WITNESSES: usize = 128;

fn refuse() -> Error {
    Error::InvalidConsignment
}

fn mined_position(status: &TxStatus, tip: u32, required: u8) -> Result<WitnessOrd, Error> {
    let height = status.block_height.ok_or_else(refuse)?;
    let time = status.block_time.ok_or_else(refuse)?;
    if !status.confirmed || status.block_hash.is_none() || required == 0 {
        return Err(refuse());
    }
    let confirmations = tip
        .checked_sub(height)
        .and_then(|depth| depth.checked_add(1))
        .ok_or_else(refuse)?;
    if confirmations < u32::from(required) {
        return Err(refuse());
    }
    let height = NonZeroU32::new(height).ok_or_else(refuse)?;
    let time = i64::try_from(time).map_err(|_| refuse())?;
    WitnessPos::bitcoin(height, time)
        .map(WitnessOrd::Mined)
        .ok_or_else(refuse)
}

// Include the live BDK stage: snapshot_for_review carries the persisted BDK
// changeset, which alone cannot detect a changed but not yet persisted graph.
pub(super) fn complete_source(wallet: &Wallet) -> Result<serde_json::Value, Error> {
    serde_json::to_value((wallet.snapshot_for_review()?, wallet.bdk_wallet.staged()))
        .map_err(|_| refuse())
}

pub(super) struct FrozenIncoming {
    pub(super) source: serde_json::Value,
    pub(super) resolver: PreFetchResolver,
    pub(super) transfer: ValidTransfer,
    pub(super) post: super::idb_store::WalletSnapshot,
}

impl Wallet {
    /// Validate an exact pending witness receipt against one unchanged indexer
    /// chain view and complete RGB history. The transaction must be the exact
    /// independently accepted witness, including its Bitcoin witness bytes.
    /// Network work uses GETs only. Stock/BDK/DB projection is detached, without
    /// active-state mutation, persistence,
    /// sequence allocation, signing or a settlement acknowledgement. It grants
    /// no consuming authority and is deliberately not wired to execution.
    pub async fn require_confirmed_pending_witness_receive(
        &self,
        online: Online,
        recipient: &str,
        contract: ContractId,
        outpoint: OutPoint,
        units: u64,
        consignment: &[u8],
        transaction: &BdkTransaction,
    ) -> Result<(), Error> {
        self.observe_pending_witness_receive(
            online,
            recipient,
            contract,
            outpoint,
            units,
            consignment,
            transaction,
        )
        .await?;
        Ok(())
    }

    pub(super) async fn observe_pending_witness_receive(
        &self,
        online: Online,
        recipient: &str,
        contract: ContractId,
        outpoint: OutPoint,
        units: u64,
        consignment: &[u8],
        transaction: &BdkTransaction,
    ) -> Result<FrozenIncoming, Error> {
        self.check_online(online.clone())?;
        if transaction.compute_txid() != outpoint.txid
            || transaction.total_size() > 4_000_000
            || transaction.input.len() > super::incoming_bdk::MAX_INPUTS
            || transaction.output.len() > super::incoming_bdk::MAX_OUTPUTS
            || transaction.is_coinbase()
        {
            return Err(refuse());
        }
        let output = transaction
            .output
            .get(outpoint.vout as usize)
            .ok_or_else(refuse)?;
        if output.value.to_sat() == 0 {
            return Err(refuse());
        }
        self.require_exact_pending_witness_receive(
            recipient,
            contract,
            outpoint,
            units,
            consignment,
            &output.script_pubkey,
        )?;
        let source = complete_source(self)?;
        let db = self.database.get_db_data(false)?;
        let batch = db
            .batch_transfers
            .iter()
            .find(|b| b.txid.as_deref() == Some(outpoint.txid.to_string().as_str()) && b.pending())
            .ok_or_else(refuse)?;
        let transfer = RgbTransfer::load(consignment).map_err(|_| refuse())?;
        let witness_id = RgbTxid::from_str(&outpoint.txid.to_string()).map_err(|_| refuse())?;
        let mut witnesses = BTreeSet::new();
        for bundle in transfer.bundled_witnesses() {
            if !witnesses.insert(bundle.witness_id()) || witnesses.len() > MAX_HISTORY_WITNESSES {
                return Err(refuse());
            }
        }
        if !witnesses.contains(&witness_id) {
            return Err(refuse());
        }
        // Exact allocation data comes from validated transitions below; neither
        // imported DB amounts nor display metadata establish received units.
        let assignments =
            self.extract_received_assignments(&transfer, witness_id, Some(outpoint.vout), None);
        let received =
            assignments
                .values()
                .try_fold(0u64, |total, assignment| match assignment {
                    Assignment::Fungible(amount) if *amount > 0 => {
                        total.checked_add(*amount).ok_or_else(refuse)
                    }
                    _ => Err(refuse()),
                })?;
        if received != units {
            return Err(refuse());
        }
        let Indexer::EsploraAsync(client) = self.indexer();
        // Custom signet has no independently configured challenge here.
        if self.wallet_data.bitcoin_network == BitcoinNetwork::SignetCustom {
            return Err(refuse());
        }
        let genesis = client.get_block_hash(0).await.map_err(IndexerError::from)?;
        let expected_genesis = bitcoin::blockdata::constants::genesis_block(BdkNetwork::from(
            self.wallet_data.bitcoin_network,
        ))
        .block_hash();
        if genesis != expected_genesis {
            return Err(refuse());
        }
        let tip = client.get_height().await.map_err(IndexerError::from)?;
        let tip_hash = client
            .get_block_hash(tip)
            .await
            .map_err(IndexerError::from)?;
        // A detached update must preserve every existing local checkpoint. Refuse
        // a local chain that disagrees with the frozen indexer, including a
        // locally newer chain. Bound this exact consumer's network work.
        let checkpoints: Vec<_> = self.bdk_wallet.local_chain().iter_checkpoints().collect();
        if checkpoints.len() > 256 {
            return Err(refuse());
        }
        for point in checkpoints {
            if point.height() > tip
                || client
                    .get_block_hash(point.height())
                    .await
                    .map_err(IndexerError::from)?
                    != point.hash()
            {
                return Err(refuse());
            }
        }
        let mut prevouts = BTreeMap::new();
        let mut previous = HashMap::new();
        for input in &transaction.input {
            let point = input.previous_output;
            if let std::collections::hash_map::Entry::Vacant(entry) = previous.entry(point.txid) {
                let tx = client
                    .get_tx(&point.txid)
                    .await
                    .map_err(IndexerError::from)?
                    .ok_or_else(refuse)?;
                if tx.compute_txid() != point.txid || tx.total_size() > 4_000_000 {
                    return Err(refuse());
                }
                entry.insert(tx);
            }
            let output = previous
                .get(&point.txid)
                .and_then(|tx| tx.output.get(point.vout as usize))
                .ok_or_else(refuse)?;
            if prevouts.insert(point, output.clone()).is_some() {
                return Err(refuse());
            }
        }
        let mut inclusion = None;
        let mut cache = HashMap::new();
        let mut statuses = Vec::new();
        for id in witnesses {
            let txid = Txid::from_str(&id.to_string()).map_err(|_| refuse())?;
            let tx = client
                .get_tx(&txid)
                .await
                .map_err(IndexerError::from)?
                .ok_or_else(refuse)?;
            let status = client
                .get_tx_status(&txid)
                .await
                .map_err(IndexerError::from)?;
            let required = if id == witness_id {
                batch.min_confirmations
            } else {
                1
            };
            let position = mined_position(&status, tip, required)?;
            if tx.compute_txid() != txid || (id == witness_id && tx != *transaction) {
                return Err(refuse());
            }
            if client
                .get_block_hash(status.block_height.ok_or_else(refuse)?)
                .await
                .map_err(IndexerError::from)?
                != status.block_hash.ok_or_else(refuse)?
            {
                return Err(refuse());
            }
            if id == witness_id {
                inclusion = Some(ConfirmationBlockTime {
                    block_id: BlockId {
                        height: status.block_height.ok_or_else(refuse)?,
                        hash: status.block_hash.ok_or_else(refuse)?,
                    },
                    confirmation_time: status.block_time.ok_or_else(refuse)?,
                });
            }
            cache.insert(id, WitnessStatus::Resolved(tx, position));
            statuses.push((txid, status));
        }
        let resolver = PreFetchResolver::new(cache, self.chain_net());
        let config = ValidationConfig {
            chain_net: self.chain_net(),
            trusted_typesystem: AssetSchema::Nia.types(),
            ..Default::default()
        };
        let validated = transfer
            .validate(&resolver, &config)
            .map_err(|_| refuse())?;
        if !super::incoming_history::safe_to_accept(validated.validation_status()) {
            return Err(refuse());
        }
        // Recheck every observed inclusion and the chain tip. Any movement is a
        // refusal, not an inferred stable view or an automatic retry.
        for (txid, status) in statuses {
            if client
                .get_tx_status(&txid)
                .await
                .map_err(IndexerError::from)?
                != status
            {
                return Err(refuse());
            }
        }
        if client.get_height().await.map_err(IndexerError::from)? != tip
            || client
                .get_block_hash(tip)
                .await
                .map_err(IndexerError::from)?
                != tip_hash
            || client.get_block_hash(0).await.map_err(IndexerError::from)? != genesis
        {
            return Err(refuse());
        }
        self.check_online(online)?;
        if complete_source(self)? != source {
            return Err(refuse());
        }
        let bdk_post = super::incoming_bdk::project(
            self,
            transaction,
            &prevouts,
            inclusion.ok_or_else(refuse)?,
        )?;
        let post = super::incoming_post::project(
            self,
            recipient,
            contract,
            outpoint,
            units,
            transaction,
            &validated,
            &resolver,
            bdk_post,
            &source,
        )?;
        Ok(FrozenIncoming {
            source,
            resolver,
            transfer: validated,
            post,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn status() -> TxStatus {
        TxStatus {
            confirmed: true,
            block_height: Some(20),
            block_hash: Some(BlockHash::from_str(&"01".repeat(32)).unwrap()),
            block_time: Some(1_700_000_000),
        }
    }

    #[test]
    fn mined_position_checks_depth_and_all_status_fields() {
        assert!(matches!(
            mined_position(&status(), 21, 2),
            Ok(WitnessOrd::Mined(_))
        ));
        assert!(mined_position(&status(), 20, 2).is_err());
        assert!(mined_position(&status(), 19, 1).is_err());
        assert!(mined_position(&status(), u32::MAX, 1).is_ok());
        assert!(mined_position(&status(), 21, 0).is_err());
        let changes: Vec<fn(&mut TxStatus)> = vec![
            |s| s.confirmed = false,
            |s| s.block_height = None,
            |s| s.block_height = Some(0),
            |s| s.block_hash = None,
            |s| s.block_time = None,
            |s| s.block_time = Some(u64::MAX),
        ];
        for change in changes {
            let mut value = status();
            change(&mut value);
            assert!(mined_position(&value, 21, 1).is_err());
        }
    }

    #[test]
    fn complete_source_detects_unpersisted_bdk_stage_without_allocating_sequence() {
        let mut wallet = super::super::offline::address_reuse_tests::make_test_wallet(false);
        *wallet.rgb_stock.borrow_mut() = Some(Stock::in_memory());
        let original_snapshot =
            serde_json::to_value(wallet.snapshot_for_review().unwrap()).unwrap();
        let original = complete_source(&wallet).unwrap();
        assert_eq!(complete_source(&wallet).unwrap(), original);
        wallet
            .bdk_wallet
            .reveal_next_address(KeychainKind::External);
        assert_eq!(
            serde_json::to_value(wallet.snapshot_for_review().unwrap()).unwrap(),
            original_snapshot
        );
        assert_ne!(complete_source(&wallet).unwrap(), original);
        let after = complete_source(&wallet).unwrap();
        assert_eq!(complete_source(&wallet).unwrap(), after);
    }

    #[test]
    fn missing_frozen_witness_never_becomes_tentative() {
        let resolver = PreFetchResolver::new(HashMap::new(), ChainNet::BitcoinRegtest);
        let id = RgbTxid::from_str(&"01".repeat(32)).unwrap();
        assert!(
            matches!(resolver.resolve_witness(id), Err(WitnessResolverError::ResolverIssue(Some(missing), _)) if missing == id)
        );
        assert!(resolver.check_chain_net(ChainNet::BitcoinRegtest).is_ok());
        assert!(resolver.check_chain_net(ChainNet::BitcoinMainnet).is_err());
    }
}
