//! Detached exact-transaction BDK projection. No active wallet or persistence writes.
use super::*;
use bdk_core::Merge;
use bdk_wallet::chain::{BlockId, ConfirmationBlockTime, local_chain::CheckPoint};

pub(super) const MAX_INPUTS: usize = 128;
pub(super) const MAX_OUTPUTS: usize = 128;

fn refuse() -> Error {
    Error::InvalidConsignment
}

/// This is a prospective BDK result only, never a consuming/approval capability.
/// The caller must independently freeze and verify the chain and every prevout.
pub(super) fn project(
    wallet: &Wallet,
    transaction: &BdkTransaction,
    prevouts: &BTreeMap<OutPoint, TxOut>,
    inclusion: ConfirmationBlockTime,
) -> Result<ChangeSet, Error> {
    project_with_update(wallet, transaction, prevouts, inclusion).map(|(post, _)| post)
}

pub(super) fn project_with_update(
    wallet: &Wallet,
    transaction: &BdkTransaction,
    prevouts: &BTreeMap<OutPoint, TxOut>,
    inclusion: ConfirmationBlockTime,
) -> Result<(ChangeSet, Update), Error> {
    if transaction.is_coinbase()
        || transaction.input.is_empty()
        || transaction.input.len() > MAX_INPUTS
        || transaction.output.is_empty()
        || transaction.output.len() > MAX_OUTPUTS
        || prevouts.len() != transaction.input.len()
        || inclusion.block_id.height == 0
    {
        return Err(refuse());
    }
    let mut source = wallet.bdk_database.get_data().clone().ok_or_else(refuse)?;
    if let Some(stage) = wallet.bdk_wallet.staged() {
        source.merge(stage.clone());
    }
    // Load public descriptors only. No key extraction, signer, network or
    // persistence backend is attached to the detached BDK owner.
    let mut detached = BdkWallet::load()
        .load_wallet_no_persist(source.clone())
        .map_err(|_| refuse())?
        .ok_or_else(refuse)?;
    let before: BTreeMap<_, _> = detached.list_output().map(|o| (o.outpoint, o)).collect();
    let active: BTreeMap<_, _> = wallet
        .bdk_wallet
        .list_output()
        .map(|o| (o.outpoint, o))
        .collect();
    if before != active {
        return Err(refuse());
    }
    let input_total = prevouts.values().try_fold(0u64, |sum, output| {
        sum.checked_add(output.value.to_sat())
            .filter(|amount| *amount <= BdkAmount::MAX_MONEY.to_sat())
            .ok_or_else(refuse)
    })?;
    let output_total = transaction.output.iter().try_fold(0u64, |sum, output| {
        sum.checked_add(output.value.to_sat())
            .filter(|amount| *amount <= BdkAmount::MAX_MONEY.to_sat())
            .ok_or_else(refuse)
    })?;
    if output_total > input_total {
        return Err(refuse());
    }
    let mut local_inputs = BTreeSet::new();
    let mut inputs = BTreeSet::new();
    for input in &transaction.input {
        let outpoint = input.previous_output;
        if !inputs.insert(outpoint) {
            return Err(refuse());
        }
        let prevout = prevouts.get(&outpoint).ok_or_else(refuse)?;
        if let Some(known) = detached.tx_graph().get_txout(outpoint) {
            if known != prevout {
                return Err(refuse());
            }
        }
        if detached.is_mine(prevout.script_pubkey.clone()) {
            let output = before.get(&outpoint).ok_or_else(refuse)?;
            if output.is_spent || output.txout != *prevout {
                return Err(refuse());
            }
            local_inputs.insert(outpoint);
        } else if before.contains_key(&outpoint) {
            return Err(refuse());
        }
    }
    let txid = transaction.compute_txid();
    if detached.tx_graph().get_tx(txid).is_some() {
        // Already observed/application-ambiguous transactions need recovery,
        // never a new preparation guessed from an existing graph entry.
        return Err(refuse());
    }
    let expected_outputs: BTreeMap<_, _> = transaction
        .output
        .iter()
        .enumerate()
        .filter(|(_, output)| detached.is_mine(output.script_pubkey.clone()))
        .map(|(vout, output)| (OutPoint::new(txid, vout as u32), output))
        .collect();
    if expected_outputs.is_empty() {
        return Err(refuse());
    }
    // Preserve all known checkpoints. The observer verified each against the
    // frozen configured chain; never replace or infer a reorg here.
    let mut blocks: BTreeMap<_, _> = detached
        .local_chain()
        .iter_checkpoints()
        .map(|point| (point.height(), point.hash()))
        .collect();
    if let Some(old) = blocks.insert(inclusion.block_id.height, inclusion.block_id.hash) {
        if old != inclusion.block_id.hash {
            return Err(refuse());
        }
    }
    let chain = CheckPoint::from_block_ids(
        blocks
            .into_iter()
            .map(|(height, hash)| BlockId { height, hash }),
    )
    .map_err(|_| refuse())?;
    let mut update = Update {
        chain: Some(chain),
        ..Default::default()
    };
    update.tx_update.txs.push(Arc::new(transaction.clone()));
    update.tx_update.anchors.insert((inclusion, txid));
    detached
        .apply_update(update.clone())
        .map_err(|_| refuse())?;
    let after: BTreeMap<_, _> = detached.list_output().map(|o| (o.outpoint, o)).collect();
    // Every existing canonical output survives exactly, except the exact local
    // inputs' spent bit. Reject conflict/eviction effects on unrelated outputs.
    for (outpoint, output) in &before {
        let mut expected = output.clone();
        if local_inputs.contains(outpoint) {
            expected.is_spent = true;
        }
        if after.get(outpoint) != Some(&expected) {
            return Err(refuse());
        }
    }
    if after.len()
        != before
            .len()
            .checked_add(expected_outputs.len())
            .ok_or_else(refuse)?
    {
        return Err(refuse());
    }
    for (outpoint, expected) in expected_outputs {
        let actual = after.get(&outpoint).ok_or_else(refuse)?;
        if actual.txout != *expected || actual.is_spent {
            return Err(refuse());
        }
    }
    if let Some(stage) = detached.take_staged() {
        source.merge(stage);
    }
    Ok((source, update))
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use bdk_wallet::bitcoin::{TxIn, absolute, transaction};

    fn transaction(inputs: Vec<OutPoint>, outputs: Vec<TxOut>) -> BdkTransaction {
        BdkTransaction {
            version: transaction::Version::TWO,
            lock_time: absolute::LockTime::ZERO,
            input: inputs
                .into_iter()
                .map(|previous_output| TxIn {
                    previous_output,
                    ..Default::default()
                })
                .collect(),
            output: outputs,
        }
    }
    fn output(script: ScriptBuf, value: u64) -> TxOut {
        TxOut {
            value: BdkAmount::from_sat(value),
            script_pubkey: script,
        }
    }
    fn fixture_with_funding(
        confirmed: bool,
    ) -> (
        Wallet,
        BdkTransaction,
        BTreeMap<OutPoint, TxOut>,
        ConfirmationBlockTime,
    ) {
        let mut wallet = super::super::offline::address_reuse_tests::make_test_wallet(false);
        *wallet.rgb_stock.borrow_mut() = Some(Stock::in_memory());
        let funding_script = wallet
            .bdk_wallet
            .reveal_next_address(KeychainKind::Internal)
            .address
            .script_pubkey();
        let unrelated_script = wallet
            .bdk_wallet
            .reveal_next_address(KeychainKind::External)
            .address
            .script_pubkey();
        let receive_script = wallet
            .bdk_wallet
            .reveal_next_address(KeychainKind::External)
            .address
            .script_pubkey();
        let change_script = wallet
            .bdk_wallet
            .reveal_next_address(KeychainKind::Internal)
            .address
            .script_pubkey();
        let funding = transaction(
            vec![OutPoint::new(Txid::from_str(&"04".repeat(32)).unwrap(), 0)],
            vec![
                output(funding_script, 10_000),
                output(unrelated_script, 3_000),
            ],
        );
        let point = OutPoint::new(funding.compute_txid(), 0);
        if confirmed {
            let funding_block = BlockId {
                height: 10,
                hash: BlockHash::from_str(&"05".repeat(32)).unwrap(),
            };
            let mut update = Update {
                chain: Some(
                    wallet
                        .bdk_wallet
                        .local_chain()
                        .tip()
                        .push(funding_block)
                        .unwrap(),
                ),
                ..Default::default()
            };
            update.tx_update.txs.push(Arc::new(funding.clone()));
            update.tx_update.anchors.insert((
                ConfirmationBlockTime {
                    block_id: funding_block,
                    confirmation_time: 1_699_999_999,
                },
                funding.compute_txid(),
            ));
            wallet.bdk_wallet.apply_update(update).unwrap();
        } else {
            wallet
                .bdk_wallet
                .apply_unconfirmed_txs([(funding.clone(), 1)]);
        }
        let external_point = OutPoint::new(Txid::from_str(&"03".repeat(32)).unwrap(), 0);
        let prevouts = BTreeMap::from([
            (point, funding.output[0].clone()),
            (external_point, output(ScriptBuf::new(), 5_000)),
        ]);
        let tx = transaction(
            vec![point, external_point],
            vec![
                output(receive_script, 1_000),
                output(change_script, 6_000),
                output(ScriptBuf::new(), 7_000),
            ],
        );
        let inclusion = ConfirmationBlockTime {
            block_id: BlockId {
                height: 20,
                hash: BlockHash::from_str(&"01".repeat(32)).unwrap(),
            },
            confirmation_time: 1_700_000_000,
        };
        (wallet, tx, prevouts, inclusion)
    }
    pub(in crate::wallet) fn fixture() -> (
        Wallet,
        BdkTransaction,
        BTreeMap<OutPoint, TxOut>,
        ConfirmationBlockTime,
    ) {
        fixture_with_funding(true)
    }
    #[test]
    fn unconfirmed_ancestor_promotion_is_refused_without_masking_unrelated_positions() {
        let (wallet, tx, prevouts, inclusion) = fixture_with_funding(false);
        let before = source(&wallet);
        assert!(project(&wallet, &tx, &prevouts, inclusion).is_err());
        assert_eq!(source(&wallet), before);
    }
    fn source(wallet: &Wallet) -> serde_json::Value {
        serde_json::to_value((
            wallet.snapshot_for_review().unwrap(),
            wallet.bdk_wallet.staged(),
        ))
        .unwrap()
    }

    #[test]
    fn real_detached_bdk_accounts_funding_receipt_change_and_preserves_unrelated_output() {
        let (wallet, tx, prevouts, inclusion) = fixture();
        let before = source(&wallet);
        let post = project(&wallet, &tx, &prevouts, inclusion).unwrap();
        assert_eq!(
            source(&wallet),
            before,
            "active complete state and stage stay unchanged"
        );
        let detached = BdkWallet::load()
            .load_wallet_no_persist(post.clone())
            .unwrap()
            .unwrap();
        let outputs: BTreeMap<_, _> = detached.list_output().map(|o| (o.outpoint, o)).collect();
        assert!(outputs[&tx.input[0].previous_output].is_spent);
        let receive = &outputs[&OutPoint::new(tx.compute_txid(), 0)];
        let change = &outputs[&OutPoint::new(tx.compute_txid(), 1)];
        assert_eq!(receive.keychain, KeychainKind::External);
        assert_eq!(change.keychain, KeychainKind::Internal);
        assert_eq!(receive.txout.value.to_sat(), 1_000);
        assert_eq!(change.txout.value.to_sat(), 6_000);
        assert_eq!(outputs.len(), 4);
        for old in wallet
            .bdk_wallet
            .list_output()
            .filter(|o| o.outpoint != tx.input[0].previous_output)
        {
            assert_eq!(outputs.get(&old.outpoint), Some(&old));
        }
        assert!(matches!(
            receive.chain_position,
            ChainPosition::Confirmed { .. }
        ));
        assert_eq!(
            post,
            project(&wallet, &tx, &prevouts, inclusion).unwrap(),
            "deterministic complete changeset"
        );
        assert_eq!(source(&wallet), before);
    }

    #[test]
    fn missing_wrong_duplicate_and_unproven_local_inputs_refuse_without_mutation() {
        let (wallet, tx, prevouts, inclusion) = fixture();
        let before = source(&wallet);
        let mut missing = prevouts.clone();
        missing.remove(&tx.input[1].previous_output);
        assert!(project(&wallet, &tx, &missing, inclusion).is_err());
        let mut wrong = prevouts.clone();
        wrong.get_mut(&tx.input[0].previous_output).unwrap().value = BdkAmount::from_sat(9_999);
        assert!(project(&wallet, &tx, &wrong, inclusion).is_err());
        let mut unknown = prevouts.clone();
        unknown
            .get_mut(&tx.input[1].previous_output)
            .unwrap()
            .script_pubkey = tx.output[0].script_pubkey.clone();
        assert!(project(&wallet, &tx, &unknown, inclusion).is_err());
        let mut duplicate = tx.clone();
        duplicate.input.push(tx.input[0].clone());
        assert!(project(&wallet, &duplicate, &prevouts, inclusion).is_err());
        let mut excessive = tx.clone();
        excessive.output[0].value = BdkAmount::from_sat(15_001);
        assert!(project(&wallet, &excessive, &prevouts, inclusion).is_err());
        let mut overflow = prevouts.clone();
        overflow
            .get_mut(&tx.input[1].previous_output)
            .unwrap()
            .value = BdkAmount::from_sat(u64::MAX);
        assert!(project(&wallet, &tx, &overflow, inclusion).is_err());
        assert_eq!(source(&wallet), before);
    }

    #[test]
    fn existing_transaction_spent_input_and_chain_conflict_require_reconciliation() {
        let (mut wallet, tx, prevouts, inclusion) = fixture();
        let mut conflicting = inclusion;
        conflicting.block_id.height = 0;
        assert!(project(&wallet, &tx, &prevouts, conflicting).is_err());
        let mut chain = wallet.bdk_wallet.local_chain().tip();
        chain = chain
            .push(BlockId {
                height: 20,
                hash: BlockHash::from_str(&"02".repeat(32)).unwrap(),
            })
            .unwrap();
        wallet
            .bdk_wallet
            .apply_update(Update {
                chain: Some(chain),
                ..Default::default()
            })
            .unwrap();
        let before = source(&wallet);
        assert!(project(&wallet, &tx, &prevouts, inclusion).is_err());
        assert_eq!(source(&wallet), before);
        let (mut wallet, tx, prevouts, inclusion) = fixture();
        wallet.bdk_wallet.apply_unconfirmed_txs([(tx.clone(), 2)]);
        let before = source(&wallet);
        assert!(project(&wallet, &tx, &prevouts, inclusion).is_err());
        assert_eq!(source(&wallet), before);
    }
}
