//! Private exact incoming application core. No approval or persistence authority.
//! Kept private until the embedding journal commits intent/debit before consume
//! and quarantines every error/cancellation after that acknowledgement.
use super::incoming_observation::{FrozenIncoming, complete_source};
use super::*;

/// Exclusive SAME-engine custody. Neither clonable nor serializable. Dropping
/// before consume leaves the engine unchanged; it does not roll back host intent.
pub(super) struct PreparedIncoming<'a> {
    wallet: &'a mut Wallet,
    frozen: FrozenIncoming,
}

impl<'a> PreparedIncoming<'a> {
    pub(super) fn new(wallet: &'a mut Wallet, frozen: FrozenIncoming) -> Result<Self, Error> {
        let engine = frozen.engine.upgrade().ok_or(Error::InvalidConsignment)?;
        if !std::rc::Rc::ptr_eq(&engine, &wallet.rgb_stock)
            || wallet.wallet_data.bitcoin_network != BitcoinNetwork::Regtest
            || !wallet.consumer_owned_persistence
            || complete_source(wallet)? != frozen.source
        {
            return Err(Error::InvalidConsignment);
        }
        wallet.use_consumer_owned_persistence()?;
        Ok(Self { wallet, frozen })
    }

    /// Memory-only, consuming transition. The host must durably acknowledge its
    /// exact intent/debit and revalidate session/expiry/revocation BEFORE calling.
    /// Success is NOT durable settlement or permission to sign. The complete
    /// resulting checkpoint still needs authenticated journal CAS/read-back.
    /// Errors after the first mutation are ambiguous; never infer rollback/retry.
    pub(super) fn apply(self) -> Result<super::idb_store::WalletSnapshot, Error> {
        let Self { wallet, frozen } = self;
        if complete_source(wallet)? != frozen.source || !wallet.consumer_owned_persistence {
            return Err(Error::InvalidConsignment);
        }
        // All fallible DB derivation occurs before the first active mutation.
        // The exact projected BDK graph is only used to derive precise DB rows;
        // the live BDK receives its established Update, never a snapshot import.
        let after = BdkWallet::load()
            .load_wallet_no_persist(
                frozen
                    .post
                    .bdk_changeset
                    .clone()
                    .ok_or(Error::InvalidConsignment)?,
            )
            .map_err(|_| Error::InvalidConsignment)?
            .ok_or(Error::InvalidConsignment)?;
        let data = wallet.database.get_db_data(false)?;
        let batch = data
            .batch_transfers
            .iter()
            .find(|b| {
                b.txid.as_deref() == Some(frozen.receipt.txid.to_string().as_str()) && b.pending()
            })
            .ok_or(Error::InvalidConsignment)?;
        let db = super::incoming_post::project_db(
            &wallet.database,
            &wallet.bdk_wallet,
            &after,
            &frozen.transaction,
            frozen.receipt,
            batch,
        )?;
        if serde_json::to_value(&db).map_err(|_| Error::InvalidConsignment)?
            != serde_json::to_value(&frozen.post.db).map_err(|_| Error::InvalidConsignment)?
        {
            return Err(Error::InvalidConsignment);
        }
        // Preserve the existing DB owner; a shared alias prevents exclusive
        // application and must refuse before Stock/BDK mutation.
        let database = Arc::get_mut(&mut wallet.database).ok_or(Error::InvalidConsignment)?;
        wallet
            .rgb_stock
            .borrow_mut()
            .as_mut()
            .ok_or(Error::InvalidConsignment)?
            .accept_transfer(frozen.transfer, &frozen.resolver)
            .map_err(|_| Error::InvalidConsignment)?;
        wallet
            .bdk_wallet
            .apply_update(frozen.update)
            .map_err(|_| Error::InvalidConsignment)?;
        wallet
            .bdk_wallet
            .persist(&mut wallet.bdk_database)
            .map_err(|_| Error::InvalidConsignment)?;
        *database = db;
        wallet
            .received_consignments
            .remove(&frozen.recipient)
            .ok_or(Error::InvalidConsignment)?;
        let actual = wallet.snapshot_for_review()?;
        if serde_json::to_value(&actual).map_err(|_| Error::InvalidConsignment)?
            != serde_json::to_value(frozen.post).map_err(|_| Error::InvalidConsignment)?
        {
            return Err(Error::InvalidConsignment);
        }
        Ok(actual)
    }
}
