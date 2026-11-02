//! Ordinary Bitcoin eligibility is derived from the real BDK graph, RGB database
//! and every contract in Stock. An external-keychain output is not necessarily
//! an RGB asset, and an empty displayed allocation is not proof that it is free.
//! No assignment, reservation, transfer or balance is reconstructed or removed.
use super::*;

const MAX_INPUTS: usize = 4096;
const MAX_PSBT_INPUTS: usize = 128;
const MAX_ROWS: usize = 32768;

/// Index the bounded native rows once; repeated balance polling must not scan
/// the whole database for every coin or accept duplicate persisted identities.
struct BitcoinDbInputs<'a> {
    txos: HashMap<OutPoint, &'a DbTxo>,
    colored: HashSet<i32>,
    blinded: HashSet<OutPoint>,
    pending_scripts: HashSet<String>,
}

impl<'a> BitcoinDbInputs<'a> {
    fn new(db: &'a DbData, pending_scripts: HashSet<String>) -> Result<Self, Error> {
        let mut txos = HashMap::new();
        let mut ids = HashSet::new();
        for row in &db.txos {
            let txid = Txid::from_str(&row.txid).map_err(|_| refused())?;
            if txid.to_string() != row.txid
                || row.idx <= 0
                || !ids.insert(row.idx)
                || txos
                    .insert(
                        OutPoint {
                            txid,
                            vout: row.vout,
                        },
                        row,
                    )
                    .is_some()
            {
                return Err(refused());
            }
        }
        let mut blinded = HashSet::new();
        for transfer in &db.transfers {
            if let Some(RecipientTypeFull::Blind { unblinded_utxo }) = &transfer.recipient_type {
                let txid = Txid::from_str(&unblinded_utxo.txid).map_err(|_| refused())?;
                if txid.to_string() != unblinded_utxo.txid {
                    return Err(refused());
                }
                blinded.insert(OutPoint {
                    txid,
                    vout: unblinded_utxo.vout,
                });
            }
        }
        Ok(Self {
            txos,
            colored: db.colorings.iter().map(|c| c.txo_idx).collect(),
            blinded,
            pending_scripts,
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
/// A confirmed native coin with no Stock assignment or pending receive promise.
pub struct BitcoinSpendInput {
    /// Canonical Bitcoin transaction ID and output index.
    pub outpoint: String,
    /// Whole satoshis encoded as an unsigned decimal string.
    pub amount_sats: String,
    /// Exact BDK-owned output script.
    pub script_hex: String,
    /// Descriptor keychain, either `vanilla` or `colored`.
    pub keychain: String,
}

/// Public, current native observation. Never an authorization or cached balance.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BitcoinSpendView {
    /// Explicit response version, currently one.
    pub version: u8,
    /// Fixed `wallet-bitcoin-spendability` response discriminator.
    pub kind: String,
    /// This implementation is REGTEST-only.
    pub network: String,
    /// Height of the native BDK observation.
    pub tip_height: u32,
    /// Exact block hash of that observation.
    pub tip_hash: String,
    /// Checked sum of the qualified inputs, in integer satoshis.
    pub total_sats: String,
    /// Native inputs; caller-owned refusal reservations are applied separately.
    pub inputs: Vec<BitcoinSpendInput>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
/// Bounded customer request, using only whole integer monetary values.
pub struct BitcoinSpendRequest {
    /// Explicit request version, currently one.
    pub version: u8,
    /// Exact network-checked destination address.
    pub address: String,
    /// Whole satoshis delivered to the destination.
    pub amount_sats: String,
    /// Whole satoshis per virtual byte, bounded to 1 through 1000.
    pub fee_rate: String,
    /// Unix seconds, at most 120 seconds into the future.
    pub expires_at: String,
    /// Additional caller-owned durable input reservations.
    pub excluded_inputs: Vec<String>,
}

fn refused() -> Error {
    Error::InvalidPsbt {
        details:
            "Ordinary Bitcoin inputs cannot be verified against current native wallet and RGB state"
                .into(),
    }
}

fn integer(value: &str) -> Result<u64, Error> {
    let parsed: u64 = value.parse().map_err(|_| refused())?;
    if parsed.to_string() != value {
        return Err(refused());
    }
    Ok(parsed)
}

fn unix_seconds() -> Result<u64, Error> {
    #[cfg(target_arch = "wasm32")]
    let timestamp = now().unix_timestamp();
    #[cfg(not(target_arch = "wasm32"))]
    let timestamp = time::OffsetDateTime::now_utc().unix_timestamp();
    u64::try_from(timestamp).map_err(|_| refused())
}

fn unexpired(expires_at: &str) -> Result<(), Error> {
    let expiry = integer(expires_at)?;
    let now = unix_seconds()?;
    // A caller cannot give this narrow ordinary-send API an unbounded approval.
    if expiry <= now || expiry > now.checked_add(120).ok_or_else(refused)? {
        return Err(refused());
    }
    Ok(())
}

fn validate_bitcoin_request(
    request: &BitcoinSpendRequest,
) -> Result<(u64, u64, HashSet<OutPoint>), Error> {
    if request.version != 1
        || request.address.len() > 100
        || request.address.trim() != request.address
        || request.excluded_inputs.len() > MAX_INPUTS
    {
        return Err(refused());
    }
    let amount = integer(&request.amount_sats)?;
    let rate = integer(&request.fee_rate)?;
    if amount == 0 || amount > BdkAmount::MAX_MONEY.to_sat() || !(1..=1000).contains(&rate) {
        return Err(refused());
    }
    unexpired(&request.expires_at)?;
    let mut excluded = HashSet::new();
    for encoded in &request.excluded_inputs {
        let point = OutPoint::from_str(encoded).map_err(|_| refused())?;
        if point.to_string() != *encoded || !excluded.insert(point) {
            return Err(refused());
        }
    }
    unexpired(&request.expires_at)?;
    Ok((amount, rate, excluded))
}

impl Wallet {
    /// Preserve native originals' fee and asset inputs, including external atomic
    /// proposals. Legacy preparations whose fee inputs are not retained are not
    /// safe to classify; fail closed rather than infer their selection.
    fn bitcoin_native_holds(
        &self,
        db: &DbData,
        own_rgb: Option<&str>,
    ) -> Result<HashSet<OutPoint>, Error> {
        if self.transfer_artifacts.len() > MAX_INPUTS {
            return Err(refused());
        }
        let mut asset_batches = HashMap::new();
        for asset in &db.asset_transfers {
            if asset.idx <= 0
                || asset_batches
                    .insert(asset.idx, asset.batch_transfer_idx)
                    .is_some()
            {
                return Err(refused());
            }
        }
        let mut outgoing_batches = HashSet::new();
        for transfer in &db.transfers {
            if !transfer.incoming {
                outgoing_batches.insert(
                    *asset_batches
                        .get(&transfer.asset_transfer_idx)
                        .ok_or_else(refused)?,
                );
            }
        }
        let mut holds = HashSet::new();
        for batch in &db.batch_transfers {
            let external = batch
                .txid
                .as_ref()
                .and_then(|id| self.transfer_artifacts.get(id))
                .and_then(|a| a.external_send.as_ref());
            if batch.status == TransferStatus::Initiated && external.is_none() {
                return Err(refused());
            }
            if !batch.status.settled()
                && outgoing_batches.contains(&batch.idx)
                && external.is_none()
            {
                // Older unmanaged preparations do not retain an unsigned fee
                // selection. A retained signed original can be fenced exactly;
                // without it the inputs are unknown, not released by expiry.
                let id = batch.txid.as_ref().ok_or_else(refused)?;
                let bytes = self
                    .transfer_artifacts
                    .get(id)
                    .and_then(|a| a.signed_psbt.as_ref())
                    .ok_or_else(refused)?;
                if bytes.len() > 524_288 {
                    return Err(refused());
                }
                let original = Psbt::from_str(bytes).map_err(|_| refused())?;
                if original.unsigned_tx.compute_txid().to_string() != *id
                    || original.inputs.is_empty()
                    || original.inputs.len() > MAX_PSBT_INPUTS
                {
                    return Err(refused());
                }
                holds.extend(original.unsigned_tx.input.iter().map(|i| i.previous_output));
            }
        }
        for (txid, artifacts) in &self.transfer_artifacts {
            let Some(external) = &artifacts.external_send else {
                continue;
            };
            let batch = Self::outgoing_batch_from_data(db, txid)?.ok_or_else(refused)?;
            if external.prepared_psbt.len() > 524_288 {
                return Err(refused());
            }
            let original = Psbt::from_str(&external.prepared_psbt).map_err(|_| refused())?;
            if original.unsigned_tx.compute_txid().to_string() != *txid
                || Self::tx_stripped_hex_from_unsigned(&original.unsigned_tx)
                    != external.expected_tx_hex
                || original.unsigned_tx.input.is_empty()
                || original.unsigned_tx.input.len() > MAX_PSBT_INPUTS
            {
                return Err(refused());
            }
            if own_rgb == Some(txid.as_str()) {
                if external.wallet_transfer.is_none()
                    || external.status != ExternalSendStatus::Prepared
                    || batch.status != TransferStatus::Initiated
                {
                    return Err(refused());
                }
                // Only this read-only fee-evidence path permits the exact
                // original's own input. Ordinary BTC preparation/signing and
                // broadcast always call with no exemption.
                continue;
            }
            if external.status == ExternalSendStatus::Cancelled {
                if !batch.status.failed()
                    || artifacts.signed_psbt.is_some()
                    || external
                        .wallet_transfer
                        .as_ref()
                        .is_some_and(|b| b.signing_attempted || b.broadcast_attempted || b.restored)
                {
                    return Err(refused());
                }
                continue;
            }
            if external.status == ExternalSendStatus::Conflicted {
                if !batch.status.failed()
                    || external.conflict_txid.as_ref().is_none_or(|conflict| {
                        conflict == txid || Txid::from_str(conflict).is_err()
                    })
                {
                    return Err(refused());
                }
                continue;
            }
            holds.extend(original.unsigned_tx.input.iter().map(|i| i.previous_output));
        }
        Ok(holds)
    }

    /// Database eligibility alone is insufficient: Stock is checked separately
    /// below. Reject any coloring (including zero/unknown assignments), blinded
    /// receive promise, pending witness, missing or contradictory external TXO.
    fn bitcoin_db_eligible(db: &BitcoinDbInputs<'_>, output: &LocalOutput) -> Result<bool, Error> {
        let row = db.txos.get(&output.outpoint);
        if (output.keychain == KeychainKind::External && row.is_none())
            || (output.keychain == KeychainKind::Internal && row.is_some())
        {
            return Err(refused());
        }
        if db
            .pending_scripts
            .contains(&output.txout.script_pubkey.to_hex_string())
        {
            return Ok(false);
        }
        if let Some(row) = row {
            if integer(&row.btc_amount)? != output.txout.value.to_sat() || row.spent || !row.exists
            {
                return Err(refused());
            }
            if row.pending_witness || db.colored.contains(&row.idx) {
                return Ok(false);
            }
        }
        if db.blinded.contains(&output.outpoint) {
            return Ok(false);
        }
        Ok(true)
    }

    /// Read the native confirmed, mature, assignment-free Bitcoin inputs. Both
    /// descriptor keychains are considered; neither RGB collateral nor a pending
    /// operation is available. The caller still applies its own durable holds.
    pub fn bitcoin_spend_view(&self) -> Result<BitcoinSpendView, Error> {
        self.bitcoin_spend_view_impl(None)
    }

    /// Read current assignment-free fee inputs of the exact already-prepared
    /// ordinary RGB original. Never grants Bitcoin spending permission.
    pub fn bitcoin_rgb_fee_inputs(
        &self,
        operation_id: &str,
    ) -> Result<Vec<BitcoinSpendInput>, Error> {
        let (txid, artifacts) = self
            .wallet_transfer_by_operation(operation_id)?
            .ok_or_else(refused)?;
        let external = artifacts.external_send.as_ref().ok_or_else(refused)?;
        let original = Psbt::from_str(&external.prepared_psbt).map_err(|_| refused())?;
        let points = original
            .unsigned_tx
            .input
            .iter()
            .map(|i| i.previous_output.to_string())
            .collect::<HashSet<_>>();
        Ok(self
            .bitcoin_spend_view_impl(Some(txid))?
            .inputs
            .into_iter()
            .filter(|i| points.contains(&i.outpoint))
            .collect())
    }

    fn bitcoin_spend_view_impl(&self, own_rgb: Option<&str>) -> Result<BitcoinSpendView, Error> {
        if self.bitcoin_network() != BitcoinNetwork::Regtest {
            return Err(Error::InvalidRecipientNetwork);
        }
        let tip = self.bdk_wallet.latest_checkpoint();
        let genesis =
            bitcoin::blockdata::constants::genesis_block(BdkNetwork::Regtest).block_hash();
        if tip.get(0).is_none_or(|c| c.hash() != genesis) {
            return Err(Error::InvalidRecipientNetwork);
        }
        let db = self.database.get_db_data(false)?;
        if db.txos.len() > MAX_ROWS
            || db.colorings.len() > MAX_ROWS
            || db.transfers.len() > MAX_ROWS
            || db.asset_transfers.len() > MAX_ROWS
            || db.batch_transfers.len() > MAX_ROWS
        {
            return Err(refused());
        }
        let pending = self.database.iter_pending_witness_scripts()?;
        if pending.len() > MAX_INPUTS {
            return Err(refused());
        }
        let pending_scripts = pending.into_iter().map(|p| p.script).collect();
        let held = self.bitcoin_native_holds(&db, own_rgb)?;
        let database_inputs = BitcoinDbInputs::new(&db, pending_scripts)?;
        let outputs = self
            .bdk_wallet
            .list_unspent()
            .take(MAX_INPUTS + 1)
            .collect::<Vec<_>>();
        if outputs.len() > MAX_INPUTS {
            return Err(refused());
        }
        let mut eligible = BTreeMap::new();
        for output in outputs {
            let ChainPosition::Confirmed { anchor, .. } = output.chain_position else {
                continue;
            };
            if anchor.block_id.height > tip.height() || output.is_spent {
                return Err(refused());
            }
            if output.txout.value > BdkAmount::MAX_MONEY {
                return Err(refused());
            }
            let tx = self
                .bdk_wallet
                .get_tx(output.outpoint.txid)
                .ok_or_else(refused)?;
            if tx.tx_node.tx.is_coinbase()
                && u64::from(tip.height()) - u64::from(anchor.block_id.height) + 1 < 100
            {
                continue;
            }
            if output.txout.value.to_sat() == 0
                || !output.txout.script_pubkey.is_p2tr()
                || held.contains(&output.outpoint)
                || !Self::bitcoin_db_eligible(&database_inputs, &output)?
            {
                continue;
            }
            if eligible.insert(output.outpoint, output).is_some() {
                return Err(refused());
            }
        }
        // A missing DB coloring is not permission to burn an asset. Consult all
        // contracts, schemas and assignment types in actual native Stock.
        let runtime = self.rgb_runtime()?;
        let contracts = runtime
            .contracts()?
            .into_iter()
            .take(MAX_INPUTS + 1)
            .collect::<Vec<_>>();
        if contracts.len() > MAX_INPUTS {
            return Err(refused());
        }
        let points = eligible.keys().copied().collect::<Vec<_>>();
        let mut assigned = HashSet::new();
        for contract in contracts {
            for (seal, states) in
                runtime.contract_assignments_for(contract.id, points.iter().copied())?
            {
                if !states.is_empty() {
                    assigned.insert(seal.to_outpoint());
                }
            }
        }
        let mut total = 0u64;
        let mut inputs = Vec::new();
        for (point, output) in eligible {
            if assigned.contains(&point) {
                continue;
            }
            total = total
                .checked_add(output.txout.value.to_sat())
                .ok_or_else(refused)?;
            if total > BdkAmount::MAX_MONEY.to_sat() {
                return Err(refused());
            }
            inputs.push(BitcoinSpendInput {
                outpoint: point.to_string(),
                amount_sats: output.txout.value.to_sat().to_string(),
                script_hex: output.txout.script_pubkey.to_hex_string(),
                keychain: if output.keychain == KeychainKind::Internal {
                    "vanilla"
                } else {
                    "colored"
                }
                .into(),
            });
        }
        Ok(BitcoinSpendView {
            version: 1,
            kind: "wallet-bitcoin-spendability".into(),
            network: "regtest".into(),
            tip_height: tip.height(),
            tip_hash: tip.hash().to_string(),
            total_sats: total.to_string(),
            inputs,
        })
    }

    fn checked_bitcoin_psbt(&self, encoded: &str, signed: bool) -> Result<Psbt, Error> {
        if encoded.is_empty() || encoded.len() > 180_000 {
            return Err(refused());
        }
        let psbt = Psbt::from_str(encoded).map_err(|_| refused())?;
        if psbt.inputs.is_empty()
            || psbt.inputs.len() > MAX_PSBT_INPUTS
            || psbt.inputs.len() != psbt.unsigned_tx.input.len()
            || psbt.unsigned_tx.output.is_empty()
            || psbt.unsigned_tx.output.len() > 2
            || psbt
                .unsigned_tx
                .output
                .iter()
                .any(|o| o.value.to_sat() == 0 || o.script_pubkey.is_op_return())
        {
            return Err(refused());
        }
        let view = self.bitcoin_spend_view()?;
        let coins = view
            .inputs
            .into_iter()
            .map(|c| (c.outpoint.clone(), c))
            .collect::<BTreeMap<_, _>>();
        let mut seen = HashSet::new();
        for (txin, input) in psbt.unsigned_tx.input.iter().zip(&psbt.inputs) {
            let coin = coins
                .get(&txin.previous_output.to_string())
                .ok_or_else(refused)?;
            let witness = input.witness_utxo.as_ref().ok_or_else(refused)?;
            if !seen.insert(txin.previous_output)
                || witness.value.to_sat().to_string() != coin.amount_sats
                || witness.script_pubkey.to_hex_string() != coin.script_hex
                || (!signed
                    && (input.tap_key_sig.is_some()
                        || input.final_script_witness.is_some()
                        || input.final_script_sig.is_some()
                        || !input.partial_sigs.is_empty()
                        || !input.tap_script_sigs.is_empty()))
            {
                return Err(refused());
            }
        }
        self.assert_no_managed_rgb_spend(&psbt.unsigned_tx)?;
        if signed {
            let tx = psbt.clone().extract_tx().map_err(|_| refused())?;
            if tx.input.iter().any(|i| i.witness.is_empty()) {
                return Err(refused());
            }
        }
        Ok(psbt)
    }

    /// Existing BDK coin selection, limited to the independently qualified native
    /// input set. Change always returns to the ordinary Bitcoin descriptor.
    pub async fn prepare_bitcoin_spend(
        &mut self,
        online: Online,
        request: BitcoinSpendRequest,
    ) -> Result<String, Error> {
        validate_bitcoin_request(&request)?;
        self.get_script_pubkey(&request.address)?;
        self.require_wallet_rgb_regtest(&online).await?;
        self.sync_db_txos(false).await?;
        self.build_bitcoin_spend(request)
    }

    fn build_bitcoin_spend(&mut self, request: BitcoinSpendRequest) -> Result<String, Error> {
        let (amount, rate, excluded) = validate_bitcoin_request(&request)?;
        let view = self.bitcoin_spend_view()?;
        let allowed = view
            .inputs
            .iter()
            .map(|c| OutPoint::from_str(&c.outpoint).map_err(|_| refused()))
            .collect::<Result<HashSet<_>, _>>()?;
        let blocked = self
            .bdk_wallet
            .list_unspent()
            .filter(|u| !allowed.contains(&u.outpoint) || excluded.contains(&u.outpoint))
            .map(|u| u.outpoint)
            .collect::<Vec<_>>();
        let recipient = self.get_script_pubkey(&request.address)?;
        let change = self
            ._get_new_address(KeychainKind::Internal)?
            .script_pubkey();
        let fee = self._check_fee_rate(rate)?;
        let mut builder = self.bdk_wallet.build_tx();
        builder
            .unspendable(blocked)
            .add_recipient(recipient, BdkAmount::from_sat(amount))
            .drain_to(change)
            .fee_rate(fee);
        let psbt = builder.finish().map_err(|error| match error {
            bdk_wallet::error::CreateTxError::CoinSelection(InsufficientFunds {
                needed,
                available,
            }) => Error::InsufficientBitcoins {
                needed: needed.to_sat(),
                available: available.to_sat(),
            },
            bdk_wallet::error::CreateTxError::OutputBelowDustLimit(_) => {
                Error::OutputBelowDustLimit
            }
            _ => refused(),
        })?;
        let encoded = psbt.to_string();
        self.checked_bitcoin_psbt(&encoded, false)?;
        self.bdk_wallet.persist(&mut self.bdk_database)?;
        Ok(encoded)
    }

    /// The host must hold its durable original reservation before entering this
    /// method. Recheck Stock and pending receives after sync and before BDK signs.
    pub async fn sign_bitcoin_spend(
        &mut self,
        online: Online,
        encoded: String,
        expires_at: String,
    ) -> Result<String, Error> {
        unexpired(&expires_at)?;
        self.require_wallet_rgb_regtest(&online).await?;
        self.sync_db_txos(false).await?;
        unexpired(&expires_at)?;
        let mut psbt = self.checked_bitcoin_psbt(&encoded, false)?;
        self._sign_psbt(&mut psbt, None)?;
        let signed = psbt.to_string();
        self.checked_bitcoin_psbt(&signed, true)?;
        Ok(signed)
    }

    /// Exactly one explicit submission. The caller persists signed bytes and its
    /// attempt first; ambiguous results are observed, never submitted again.
    pub async fn broadcast_bitcoin_spend(
        &mut self,
        online: Online,
        encoded: String,
        expires_at: String,
    ) -> Result<String, Error> {
        unexpired(&expires_at)?;
        self.require_wallet_rgb_regtest(&online).await?;
        self.sync_db_txos(false).await?;
        unexpired(&expires_at)?;
        let psbt = self.checked_bitcoin_psbt(&encoded, true)?;
        let tx = self._broadcast_psbt(psbt, true).await?;
        self.update_backup_info(false)?;
        self.trigger_auto_backup();
        Ok(tx.compute_txid().to_string())
    }
}

#[cfg(test)]
pub(super) mod tests {
    //! Real BDK/Stock fixtures on an invented local graph, never chain funding or
    //! financial settlement. No remote RPC, consignment delivery or broadcasting.
    use super::*;
    use bdk_wallet::chain::{BlockId, ConfirmationBlockTime};

    pub(in crate::wallet::online) fn wallet() -> Wallet {
        let keys = generate_keys(BitcoinNetwork::Regtest);
        Wallet::new(WalletData {
            data_dir: "/tmp/rgb_safe_bitcoin_unit".into(),
            bitcoin_network: BitcoinNetwork::Regtest,
            database_type: DatabaseType::Sqlite,
            max_allocations_per_utxo: 5,
            account_xpub_vanilla: keys.account_xpub_vanilla,
            account_xpub_colored: keys.account_xpub_colored,
            mnemonic: Some(keys.mnemonic),
            master_fingerprint: keys.master_fingerprint,
            vanilla_keychain: None,
            supported_schemas: vec![AssetSchema::Nia, AssetSchema::Ifa],
            reuse_addresses: false,
        })
        .unwrap()
    }

    pub(in crate::wallet::online) fn coin(
        wallet: &mut Wallet,
        keychain: KeychainKind,
        value: u64,
        confirmed: bool,
        coinbase: bool,
    ) -> OutPoint {
        let script = wallet._get_new_address(keychain).unwrap().script_pubkey();
        let tx = BdkTransaction {
            version: bitcoin::transaction::Version::TWO,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: vec![bitcoin::TxIn {
                previous_output: if coinbase {
                    OutPoint::null()
                } else {
                    OutPoint {
                        txid: Txid::from_byte_array([71; 32]),
                        // Equal-value coins are independent, never accidental
                        // conflicting spends of the same invented parent.
                        vout: u32::try_from(wallet.bdk_wallet.transactions().count()).unwrap(),
                    }
                },
                ..Default::default()
            }],
            output: vec![TxOut {
                value: BdkAmount::from_sat(value),
                script_pubkey: script,
            }],
        };
        let txid = tx.compute_txid();
        let mut update = Update::default();
        update.tx_update.txs.push(Arc::new(tx));
        if confirmed {
            let tip = wallet.bdk_wallet.latest_checkpoint();
            let block = if tip.height() == 0 {
                tip.push(BlockId {
                    height: 200,
                    hash: BlockHash::from_byte_array([81; 32]),
                })
                .unwrap()
            } else {
                tip
            };
            update.tx_update.anchors.insert((
                ConfirmationBlockTime {
                    block_id: block.block_id(),
                    confirmation_time: 1,
                },
                txid,
            ));
            update.chain = Some(block);
        } else {
            update.tx_update.seen_ats.insert((txid, 1));
        }
        wallet.bdk_wallet.apply_update(update).unwrap();
        if keychain == KeychainKind::External {
            wallet
                .database
                .set_txo(DbTxoActMod {
                    txid: ActiveValue::Set(txid.to_string()),
                    vout: ActiveValue::Set(0),
                    btc_amount: ActiveValue::Set(value.to_string()),
                    exists: ActiveValue::Set(true),
                    ..Default::default()
                })
                .unwrap();
        }
        OutPoint { txid, vout: 0 }
    }

    pub(in crate::wallet::online) fn assign_stock(
        wallet: &Wallet,
        point: OutPoint,
        inflation: bool,
    ) {
        let spec = AssetSpec {
            ticker: Ticker::try_from("FAKE".to_owned()).unwrap(),
            name: Name::try_from("Clearly fake unit asset".to_owned()).unwrap(),
            details: None,
            precision: Precision::try_from(0u8).unwrap(),
        };
        let terms = ContractTerms {
            text: RicardianContract::default(),
            media: None,
        };
        let mut builder = if inflation {
            ContractBuilder::with(
                Identity::default(),
                InflatableFungibleAsset::schema(),
                InflatableFungibleAsset::types(),
                InflatableFungibleAsset::scripts(),
                wallet.chain_net(),
            )
        } else {
            ContractBuilder::with(
                Identity::default(),
                NonInflatableAsset::schema(),
                NonInflatableAsset::types(),
                NonInflatableAsset::scripts(),
                wallet.chain_net(),
            )
        }
        .add_global_state("spec", spec)
        .unwrap()
        .add_global_state("terms", terms)
        .unwrap()
        .add_global_state(
            "issuedSupply",
            Amount::from(if inflation { 0u64 } else { 3u64 }),
        )
        .unwrap();
        builder = if inflation {
            builder
                .add_global_state("maxSupply", Amount::from(7u64))
                .unwrap()
                .add_fungible_state("inflationAllowance", wallet.get_builder_seal(point), 7u64)
                .unwrap()
        } else {
            builder
                .add_fungible_state("assetOwner", wallet.get_builder_seal(point), 3u64)
                .unwrap()
        };
        let contract = builder.issue_contract().unwrap();
        wallet
            .rgb_runtime()
            .unwrap()
            .import_contract(contract, &DumbResolver)
            .unwrap();
    }

    fn request(destination: String) -> BitcoinSpendRequest {
        BitcoinSpendRequest {
            version: 1,
            address: destination,
            amount_sats: "1000".into(),
            fee_rate: "3".into(),
            expires_at: (unix_seconds().unwrap() + 60).to_string(),
            excluded_inputs: vec![],
        }
    }

    pub(in crate::wallet::online) fn retain_external(
        wallet: &mut Wallet,
        psbt: &Psbt,
        ordinary: bool,
    ) -> (String, i32) {
        let txid = psbt.unsigned_tx.compute_txid().to_string();
        // Reservation graph fixture only, not a valid RGB transfer/consignment.
        let binding = ordinary.then(|| super::super::wallet_transfer::WalletTransferBinding {
            operation_id: "01a19805-0462-759b-9050-b9b0feb9da8f".into(),
            request_json: "unit graph; no financial authorization".into(),
            signing_attempted: false,
            broadcast_attempted: false,
            restored: false,
        });
        wallet.transfer_artifacts.insert(
            txid.clone(),
            TransferArtifacts {
                external_send: Some(ExternalSendArtifact {
                    prepared_psbt: psbt.to_string(),
                    expected_tx_hex: Wallet::tx_stripped_hex_from_unsigned(&psbt.unsigned_tx),
                    fascia_json: "unfunded fixture".into(),
                    recipients_digest: "unfunded fixture".into(),
                    status: ExternalSendStatus::Prepared,
                    conflict_txid: None,
                    wallet_transfer: binding,
                }),
                ..Default::default()
            },
        );
        let batch = wallet
            .database
            .set_batch_transfer(DbBatchTransferActMod {
                txid: ActiveValue::Set(Some(txid.clone())),
                status: ActiveValue::Set(TransferStatus::Initiated),
                min_confirmations: ActiveValue::Set(1),
                created_at: ActiveValue::Set(1),
                ..Default::default()
            })
            .unwrap();
        let asset = wallet
            .database
            .set_asset_transfer(DbAssetTransferActMod {
                batch_transfer_idx: ActiveValue::Set(batch),
                user_driven: ActiveValue::Set(true),
                ..Default::default()
            })
            .unwrap();
        wallet
            .database
            .set_transfer(DbTransferActMod {
                asset_transfer_idx: ActiveValue::Set(asset),
                incoming: ActiveValue::Set(false),
                ..Default::default()
            })
            .unwrap();
        (txid, batch)
    }

    #[test]
    fn prepared_atomic_or_ordinary_rgb_fee_inputs_remain_unspendable() {
        for ordinary in [false, true] {
            let mut wallet = wallet();
            coin(&mut wallet, KeychainKind::External, 96233, true, false);
            let destination = wallet
                ._get_new_address(KeychainKind::Internal)
                .unwrap()
                .to_string();
            let encoded = wallet.build_bitcoin_spend(request(destination)).unwrap();
            let psbt = Psbt::from_str(&encoded).unwrap();
            let (txid, _) = retain_external(&mut wallet, &psbt, ordinary);
            assert_eq!(wallet.bitcoin_spend_view().unwrap().total_sats, "0");
            assert!(wallet.checked_bitcoin_psbt(&encoded, false).is_err());
            if ordinary {
                let fee_view = wallet
                    .bitcoin_rgb_fee_inputs("01a19805-0462-759b-9050-b9b0feb9da8f")
                    .unwrap();
                assert_eq!(fee_view.len(), 1);
                assert_eq!(fee_view[0].amount_sats, "96233");
                assert!(
                    wallet
                        .bitcoin_rgb_fee_inputs("01a19805-0462-759b-9050-b9b0feb9da8a")
                        .is_err()
                );
                assert_eq!(
                    wallet.bitcoin_spend_view().unwrap().total_sats,
                    "0",
                    "read evidence does not release a hold"
                );
                wallet
                    .transfer_artifacts
                    .get_mut(&txid)
                    .unwrap()
                    .external_send
                    .as_mut()
                    .unwrap()
                    .expected_tx_hex = "changed".into();
                assert!(
                    wallet
                        .bitcoin_rgb_fee_inputs("01a19805-0462-759b-9050-b9b0feb9da8f")
                        .is_err()
                );
            }
        }
    }

    #[test]
    fn cancelled_rgb_fee_input_releases_only_with_unsigned_nonrestored_failed_proof() {
        let mut wallet = wallet();
        coin(&mut wallet, KeychainKind::External, 96233, true, false);
        let destination = wallet
            ._get_new_address(KeychainKind::Internal)
            .unwrap()
            .to_string();
        let encoded = wallet.build_bitcoin_spend(request(destination)).unwrap();
        let (txid, batch_idx) =
            retain_external(&mut wallet, &Psbt::from_str(&encoded).unwrap(), true);
        wallet
            .transfer_artifacts
            .get_mut(&txid)
            .unwrap()
            .external_send
            .as_mut()
            .unwrap()
            .status = ExternalSendStatus::Cancelled;
        assert!(
            wallet.bitcoin_spend_view().is_err(),
            "status alone cannot release"
        );
        let mut batch: DbBatchTransferActMod = wallet
            .database
            .iter_batch_transfers()
            .unwrap()
            .into_iter()
            .find(|b| b.idx == batch_idx)
            .unwrap()
            .into();
        batch.status = ActiveValue::Set(TransferStatus::Failed);
        wallet.database.update_batch_transfer(&mut batch).unwrap();
        assert_eq!(wallet.bitcoin_spend_view().unwrap().total_sats, "96233");
        wallet
            .transfer_artifacts
            .get_mut(&txid)
            .unwrap()
            .external_send
            .as_mut()
            .unwrap()
            .wallet_transfer
            .as_mut()
            .unwrap()
            .restored = true;
        assert!(
            wallet.bitcoin_spend_view().is_err(),
            "restored earlier files never prove unsigned cancellation"
        );
    }

    #[test]
    fn unknown_legacy_preparation_and_missing_original_never_credit_funds() {
        let mut wallet = wallet();
        coin(&mut wallet, KeychainKind::External, 96233, true, false);
        wallet
            .database
            .set_batch_transfer(DbBatchTransferActMod {
                status: ActiveValue::Set(TransferStatus::Initiated),
                ..Default::default()
            })
            .unwrap();
        assert!(wallet.bitcoin_spend_view().is_err());
    }

    #[test]
    fn rgb_builder_separates_exact_asset_carrier_from_vanilla_bitcoin_change() {
        let mut wallet = wallet();
        let point = coin(&mut wallet, KeychainKind::External, 96233, true, false);
        let recipient = wallet
            ._get_new_address(KeychainKind::External)
            .unwrap()
            .script_pubkey();
        let asset_change = wallet
            ._get_new_address(KeychainKind::External)
            .unwrap()
            .script_pubkey();
        let bitcoin_change = wallet
            ._get_new_address(KeychainKind::Internal)
            .unwrap()
            .script_pubkey();
        let (psbt, change) = wallet
            .prepare_psbt_with_change(
                HashSet::from([point]),
                &vec![(recipient.clone(), 1000), (asset_change.clone(), 1000)],
                wallet._check_fee_rate(3).unwrap(),
                None,
                bitcoin_change.clone(),
            )
            .unwrap();
        assert_eq!(psbt.unsigned_tx.output.len(), 4);
        assert!(psbt.unsigned_tx.output[0].script_pubkey.is_op_return());
        assert_eq!(psbt.unsigned_tx.output[1].script_pubkey, recipient);
        assert_eq!(psbt.unsigned_tx.output[2].script_pubkey, asset_change);
        assert_eq!(psbt.unsigned_tx.output[2].value.to_sat(), 1000);
        assert_eq!(psbt.unsigned_tx.output[3].script_pubkey, bitcoin_change);
        assert_eq!(change.unwrap().vout, 3);
        assert!(psbt.unsigned_tx.output[3].value.to_sat() > 90000);
    }

    #[test]
    fn rgb_builder_handles_split_carriers_and_assignment_free_fee_input() {
        let mut wallet = wallet();
        let first = coin(&mut wallet, KeychainKind::External, 1000, true, false);
        let second = coin(&mut wallet, KeychainKind::External, 1000, true, false);
        let fee = coin(&mut wallet, KeychainKind::Internal, 93769, true, false);
        let contract = ContractBuilder::with(
            Identity::default(),
            NonInflatableAsset::schema(),
            NonInflatableAsset::types(),
            NonInflatableAsset::scripts(),
            wallet.chain_net(),
        )
        .add_global_state(
            "spec",
            AssetSpec {
                ticker: Ticker::try_from("FAKE".to_owned()).unwrap(),
                name: Name::try_from("Clearly fake split unit asset".to_owned()).unwrap(),
                details: None,
                precision: Precision::try_from(0u8).unwrap(),
            },
        )
        .unwrap()
        .add_global_state(
            "terms",
            ContractTerms {
                text: RicardianContract::default(),
                media: None,
            },
        )
        .unwrap()
        .add_global_state("issuedSupply", Amount::from(4u64))
        .unwrap()
        .add_fungible_state("assetOwner", wallet.get_builder_seal(first), 1u64)
        .unwrap()
        .add_fungible_state("assetOwner", wallet.get_builder_seal(second), 3u64)
        .unwrap()
        .issue_contract()
        .unwrap();
        let id = contract.contract_id();
        wallet
            .rgb_runtime()
            .unwrap()
            .import_contract(contract, &DumbResolver)
            .unwrap();
        let recipient = wallet
            ._get_new_address(KeychainKind::External)
            .unwrap()
            .script_pubkey();
        let asset_change = wallet
            ._get_new_address(KeychainKind::External)
            .unwrap()
            .script_pubkey();
        let bitcoin_change = wallet
            ._get_new_address(KeychainKind::Internal)
            .unwrap()
            .script_pubkey();
        let rate = wallet._check_fee_rate(3).unwrap();
        let outputs = vec![(recipient.clone(), 1000), (asset_change.clone(), 1000)];
        let insufficient = wallet.prepare_psbt_with_change(
            HashSet::from([first, second]),
            &outputs,
            rate,
            None,
            bitcoin_change.clone(),
        );
        assert!(
            matches!(insufficient, Err(Error::InsufficientBitcoins { .. })),
            "{insufficient:?}"
        );
        let (mut psbt, change) = wallet
            .prepare_psbt_with_change(
                HashSet::from([first, second, fee]),
                &outputs,
                rate,
                None,
                bitcoin_change.clone(),
            )
            .unwrap();
        assert_eq!(psbt.unsigned_tx.input.len(), 3);
        assert_eq!(psbt.unsigned_tx.output.len(), 4);
        assert!(psbt.unsigned_tx.output[0].script_pubkey.is_op_return());
        assert_eq!(psbt.unsigned_tx.output[1].script_pubkey, recipient);
        assert_eq!(psbt.unsigned_tx.output[2].script_pubkey, asset_change);
        assert_eq!(psbt.unsigned_tx.output[3].script_pubkey, bitcoin_change);
        assert_eq!(change.unwrap().vout, 3);
        let stock = super::super::super::idb_store::serialize_stock(
            wallet.rgb_stock.borrow().as_ref().unwrap(),
        )
        .unwrap();
        let (fascia, beneficiaries) = wallet
            .color_psbt(
                &mut psbt,
                super::super::rust_only::ColoringInfo {
                    asset_info_map: HashMap::from([(
                        id,
                        super::super::rust_only::AssetColoringInfo {
                            output_map: HashMap::from([(0, 2), (1, 2)]),
                            static_blinding: Some(1),
                        },
                    )]),
                    static_blinding: None,
                    nonce: None,
                },
            )
            .unwrap();
        let transfers = wallet
            .create_transfers_from_fascia(&fascia, &beneficiaries)
            .unwrap();
        assert_eq!(transfers.len(), 1);
        let mut bytes = Vec::new();
        transfers[0].save(&mut bytes).unwrap();
        let result = super::super::rust_only::validate_consignment_offchain(
            &bytes,
            &psbt.unsigned_tx.compute_txid().to_string(),
            BitcoinNetwork::Regtest,
        )
        .unwrap();
        assert!(result.valid, "{result:?}");
        assert_eq!(
            super::super::super::idb_store::serialize_stock(
                wallet.rgb_stock.borrow().as_ref().unwrap()
            )
            .unwrap(),
            stock
        );
    }

    #[test]
    fn native_view_recognizes_unassigned_change_and_both_owned_keychains() {
        let mut wallet = wallet();
        let external = coin(&mut wallet, KeychainKind::External, 96233, true, false);
        let internal = coin(&mut wallet, KeychainKind::Internal, 2000, true, false);
        let before = serde_json::to_value(wallet.database.as_ref()).unwrap();
        let view = wallet.bitcoin_spend_view().unwrap();
        assert_eq!(view.total_sats, "98233");
        assert_eq!(view.inputs.len(), 2);
        assert!(
            view.inputs
                .iter()
                .any(|i| i.outpoint == external.to_string() && i.keychain == "colored")
        );
        assert!(
            view.inputs
                .iter()
                .any(|i| i.outpoint == internal.to_string() && i.keychain == "vanilla")
        );
        assert_eq!(
            serde_json::to_value(wallet.database.as_ref()).unwrap(),
            before
        );
        assert_eq!(wallet.bitcoin_spend_view().unwrap(), view);
    }

    #[test]
    fn unconfirmed_and_immature_coinbase_are_not_credited() {
        let mut wallet = wallet();
        coin(&mut wallet, KeychainKind::Internal, 2000, true, false);
        coin(&mut wallet, KeychainKind::External, 10000, false, false);
        coin(&mut wallet, KeychainKind::External, 5000, true, true);
        let view = wallet.bitcoin_spend_view().unwrap();
        assert_eq!(view.total_sats, "2000");
        assert_eq!(view.inputs.len(), 1);
    }

    #[test]
    fn missing_duplicate_or_contradictory_database_rows_fail_closed() {
        let mut wallet = wallet();
        let point = coin(&mut wallet, KeychainKind::External, 96233, true, false);
        let original = wallet.database.iter_txos().unwrap()[0].clone();
        for patch in ["missing", "amount", "spent", "absent", "duplicate"] {
            wallet.database = Arc::new(crate::database::memory_db::InMemoryDb::new());
            if patch != "missing" {
                let mut row: DbTxoActMod = original.clone().into();
                if patch == "amount" {
                    row.btc_amount = ActiveValue::Set("96234".into());
                }
                if patch == "spent" {
                    row.spent = ActiveValue::Set(true);
                }
                if patch == "absent" {
                    row.exists = ActiveValue::Set(false);
                }
                wallet.database.set_txo(row).unwrap();
                if patch == "duplicate" {
                    // InMemoryDb's normal setter upserts. A hostile persisted
                    // snapshot can still contain duplicate rows; exercise that
                    // exact serialized-state corruption rather than the setter.
                    let mut serialized = serde_json::to_value(wallet.database.as_ref()).unwrap();
                    let mut duplicate = original.clone();
                    duplicate.idx = 2;
                    serialized["txos"]
                        .as_array_mut()
                        .unwrap()
                        .push(serde_json::to_value(duplicate).unwrap());
                    wallet.database = Arc::new(serde_json::from_value(serialized).unwrap());
                }
            }
            assert!(
                wallet.bitcoin_spend_view().is_err(),
                "{patch} must not credit {point}"
            );
        }
    }

    #[test]
    fn pending_witness_and_blinded_promises_are_not_plain_bitcoin() {
        let mut wallet = wallet();
        let point = coin(&mut wallet, KeychainKind::External, 96233, true, false);
        let mut row: DbTxoActMod = wallet.database.iter_txos().unwrap()[0].clone().into();
        row.pending_witness = ActiveValue::Set(true);
        wallet.database.update_txo(row.clone()).unwrap();
        assert_eq!(wallet.bitcoin_spend_view().unwrap().total_sats, "0");
        row.pending_witness = ActiveValue::Set(false);
        wallet.database.update_txo(row.clone()).unwrap();
        let script = wallet
            .bdk_wallet
            .get_utxo(point)
            .unwrap()
            .txout
            .script_pubkey
            .to_hex_string();
        wallet
            .database
            .set_pending_witness_script(DbPendingWitnessScriptActMod {
                script: ActiveValue::Set(script.clone()),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(wallet.bitcoin_spend_view().unwrap().total_sats, "0");
        wallet.database.del_pending_witness_script(script).unwrap();
        wallet
            .database
            .set_transfer(DbTransferActMod {
                recipient_type: ActiveValue::Set(Some(RecipientTypeFull::Blind {
                    unblinded_utxo: point.into(),
                })),
                incoming: ActiveValue::Set(true),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(wallet.bitcoin_spend_view().unwrap().total_sats, "0");
    }

    #[test]
    fn every_contract_and_vanilla_seal_is_checked_against_stock() {
        let mut wallet = wallet();
        let point = coin(&mut wallet, KeychainKind::Internal, 96233, true, false);
        assert_eq!(wallet.bitcoin_spend_view().unwrap().total_sats, "96233");
        assign_stock(&wallet, point, false);
        assign_stock(&wallet, point, true);
        assert_eq!(wallet.rgb_runtime().unwrap().contracts().unwrap().len(), 2);
        assert_eq!(wallet.bitcoin_spend_view().unwrap().total_sats, "0");
    }

    #[test]
    fn other_networks_unknown_values_and_amount_overflow_are_not_spendability() {
        let mut wallet = wallet();
        coin(
            &mut wallet,
            KeychainKind::External,
            BdkAmount::MAX_MONEY.to_sat() + 1,
            true,
            false,
        );
        assert!(wallet.bitcoin_spend_view().is_err());
        wallet.wallet_data.bitcoin_network = BitcoinNetwork::Mainnet;
        assert!(matches!(
            wallet.bitcoin_spend_view(),
            Err(Error::InvalidRecipientNetwork)
        ));
        for raw in ["", " 1", "+1", "0.1", "01", "-1", "18446744073709551616"] {
            assert!(integer(raw).is_err());
        }
    }

    #[test]
    fn independently_valued_inputs_are_required_and_arbitrary_or_rgb_psbts_refused() {
        let mut wallet = wallet();
        coin(&mut wallet, KeychainKind::External, 96233, true, false);
        let destination = wallet
            ._get_new_address(KeychainKind::Internal)
            .unwrap()
            .to_string();
        let encoded = wallet.build_bitcoin_spend(request(destination)).unwrap();
        let original = Psbt::from_str(&encoded).unwrap();
        let mut changed = original.clone();
        changed.inputs[0].witness_utxo.as_mut().unwrap().value += BdkAmount::from_sat(1);
        assert!(
            wallet
                .checked_bitcoin_psbt(&changed.to_string(), false)
                .is_err()
        );
        changed = original.clone();
        changed.inputs[0].witness_utxo = None;
        assert!(
            wallet
                .checked_bitcoin_psbt(&changed.to_string(), false)
                .is_err()
        );
        changed = original.clone();
        changed.unsigned_tx.input[0].previous_output.vout += 1;
        assert!(
            wallet
                .checked_bitcoin_psbt(&changed.to_string(), false)
                .is_err()
        );
        changed = original;
        changed.unsigned_tx.output[0].script_pubkey = ScriptBuf::from_bytes(vec![0x6a, 0x00]);
        assert!(
            wallet
                .checked_bitcoin_psbt(&changed.to_string(), false)
                .is_err()
        );
    }

    #[test]
    fn actual_stock_assignments_cannot_be_burned_when_database_coloring_is_missing() {
        let mut wallet = wallet();
        coin(&mut wallet, KeychainKind::External, 96233, true, false);
        assert_eq!(wallet.bitcoin_spend_view().unwrap().total_sats, "96233");
        let point = wallet.bdk_wallet.list_unspent().next().unwrap().outpoint;
        assign_stock(&wallet, point, false);
        assert_eq!(wallet.bitcoin_spend_view().unwrap().total_sats, "0");
        for a in wallet.database.iter_asset_transfers().unwrap() {
            wallet.database.del_coloring(a.idx).unwrap();
        }
        assert_eq!(
            wallet.bitcoin_spend_view().unwrap().total_sats,
            "0",
            "actual Stock survives missing display/DB allocations"
        );
    }

    #[test]
    fn inflation_rights_are_also_assignments_even_without_fungible_amounts() {
        let mut wallet = wallet();
        coin(&mut wallet, KeychainKind::External, 96233, true, false);
        let point = wallet.bdk_wallet.list_unspent().next().unwrap().outpoint;
        assign_stock(&wallet, point, true);
        assert_eq!(wallet.bitcoin_spend_view().unwrap().total_sats, "0");
    }

    #[test]
    fn real_bdk_preparation_uses_free_change_and_returns_change_to_vanilla() {
        let mut wallet = wallet();
        let point = coin(&mut wallet, KeychainKind::External, 96233, true, false);
        let destination = wallet
            ._get_new_address(KeychainKind::External)
            .unwrap()
            .to_string();
        let encoded = wallet
            .build_bitcoin_spend(request(destination.clone()))
            .unwrap();
        let psbt = wallet.checked_bitcoin_psbt(&encoded, false).unwrap();
        assert_eq!(psbt.unsigned_tx.input.len(), 1);
        assert_eq!(psbt.unsigned_tx.input[0].previous_output, point);
        assert_eq!(psbt.unsigned_tx.output.len(), 2);
        assert!(psbt.unsigned_tx.output.iter().any(|o| o.script_pubkey
            == wallet.get_script_pubkey(&destination).unwrap()
            && o.value.to_sat() == 1000));
        let recipient = wallet.get_script_pubkey(&destination).unwrap();
        let change = psbt
            .unsigned_tx
            .output
            .iter()
            .find(|o| o.script_pubkey != recipient)
            .unwrap();
        assert_eq!(
            wallet
                .bdk_wallet
                .derivation_of_spk(change.script_pubkey.clone())
                .unwrap()
                .0,
            KeychainKind::Internal
        );
        let mut signed = psbt;
        wallet._sign_psbt(&mut signed, None).unwrap();
        assert!(
            wallet
                .checked_bitcoin_psbt(&signed.to_string(), true)
                .is_ok()
        );
        assert!(
            wallet
                .checked_bitcoin_psbt(&signed.to_string(), false)
                .is_err(),
            "no repeated signature entry"
        );
    }

    #[test]
    fn late_asset_or_receive_state_refuses_previously_reviewed_psbt() {
        let mut wallet = wallet();
        coin(&mut wallet, KeychainKind::External, 96233, true, false);
        let destination = wallet
            ._get_new_address(KeychainKind::Internal)
            .unwrap()
            .to_string();
        let prepared = wallet.build_bitcoin_spend(request(destination)).unwrap();
        let point = wallet.bdk_wallet.list_unspent().next().unwrap().outpoint;
        assign_stock(&wallet, point, false);
        assert!(wallet.checked_bitcoin_psbt(&prepared, false).is_err());
        for a in wallet.database.iter_asset_transfers().unwrap() {
            wallet.database.del_coloring(a.idx).unwrap();
        }
        assert!(
            wallet.checked_bitcoin_psbt(&prepared, false).is_err(),
            "late actual Stock cannot be omitted by a caller"
        );
    }

    #[test]
    fn exclusions_and_unrecognised_or_expired_requests_do_not_prepare() {
        let mut wallet = wallet();
        let point = coin(&mut wallet, KeychainKind::External, 96233, true, false);
        let destination = wallet
            ._get_new_address(KeychainKind::Internal)
            .unwrap()
            .to_string();
        let mut excluded = request(destination.clone());
        excluded.excluded_inputs.push(point.to_string());
        assert!(wallet.build_bitcoin_spend(excluded).is_err());
        for raw in ["0", "1.1", "01", "-1", "18446744073709551616"] {
            let mut r = request(destination.clone());
            r.amount_sats = raw.into();
            assert!(wallet.build_bitcoin_spend(r).is_err(), "{raw}");
        }
        let mut expired = request(destination.clone());
        expired.expires_at = "1".into();
        assert!(wallet.build_bitcoin_spend(expired).is_err());
        let mut version = request(destination);
        version.version = 2;
        assert!(wallet.build_bitcoin_spend(version).is_err());
    }
}
