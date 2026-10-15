//! Detached complete incoming checkpoint; no active application or consent.
use super::*;
use crate::database::memory_db::InMemoryDb;
use crate::utils::PreFetchResolver;

fn refuse() -> Error {
    Error::InvalidConsignment
}

// A receiving operation must never consume locally known RGB assignments or
// reservations on its Bitcoin funding inputs, even for another contract.
fn require_uncolored_inputs(
    db: &InMemoryDb,
    stock: &Stock,
    transaction: &BdkTransaction,
) -> Result<(), Error> {
    let data = db.get_db_data(false)?;
    for input in &transaction.input {
        let point = input.previous_output;
        let rows: Vec<_> = data
            .txos
            .iter()
            .filter(|t| t.txid == point.txid.to_string() && t.vout == point.vout)
            .collect();
        if rows.len() > 1 {
            return Err(refuse());
        }
        for row in rows {
            if row.spent
                || !row.exists
                || row.pending_witness
                || data.colorings.iter().any(|c| c.txo_idx == row.idx)
            {
                return Err(refuse());
            }
            let allocations = db.get_rgb_allocations(
                vec![row.clone()],
                Some(data.colorings.clone()),
                Some(data.batch_transfers.clone()),
                Some(data.asset_transfers.clone()),
                Some(data.transfers.clone()),
            )?;
            if allocations
                .iter()
                .any(|a| a.pending_blinded != 0 || !a.rgb_allocations.is_empty())
            {
                return Err(refuse());
            }
        }
        for info in stock.contracts().map_err(InternalError::from)? {
            if stock
                .contract_assignments_for(info.id, [point])
                .map_err(InternalError::from)?
                .values()
                .any(|s| !s.is_empty())
            {
                return Err(refuse());
            }
        }
    }
    Ok(())
}

// Require exact source DB / canonical BDK correspondence before updating only
// transaction-touched External TXOs. Internal BTC change lives in BDK only.
fn project_db(
    db: &InMemoryDb,
    before: &BdkWallet,
    after: &BdkWallet,
    transaction: &BdkTransaction,
    receipt: OutPoint,
    batch: &DbBatchTransfer,
) -> Result<InMemoryDb, Error> {
    let data = db.get_db_data(false)?;
    let mut points = HashSet::new();
    for row in &data.txos {
        if !points.insert((row.txid.clone(), row.vout)) {
            return Err(refuse());
        }
    }
    let post = db.clone();
    let inputs: HashSet<_> = transaction
        .input
        .iter()
        .map(|i| i.previous_output)
        .collect();
    for output in before
        .list_output()
        .filter(|o| o.keychain == KeychainKind::External)
    {
        let row = data
            .txos
            .iter()
            .find(|t| t.txid == output.outpoint.txid.to_string() && t.vout == output.outpoint.vout)
            .ok_or_else(refuse)?;
        if row.spent != output.is_spent
            || !row.exists
            || row.pending_witness
            || row.btc_amount != output.txout.value.to_sat().to_string()
        {
            return Err(refuse());
        }
        if inputs.contains(&output.outpoint) {
            let mut changed: DbTxoActMod = row.clone().into();
            changed.spent = ActiveValue::Set(true);
            post.update_txo(changed)?;
        }
    }
    // Refuse existing RGB TXOs absent from the canonical graph. Absence is not
    // proof of a spend and must not be 'repaired' by this exact consumer.
    let canonical: HashSet<_> = before
        .list_output()
        .filter(|o| o.keychain == KeychainKind::External)
        .map(|o| (o.outpoint.txid.to_string(), o.outpoint.vout))
        .collect();
    for row in &data.txos {
        if (row.txid.clone(), row.vout) != (receipt.txid.to_string(), receipt.vout)
            && !canonical.contains(&(row.txid.clone(), row.vout))
        {
            return Err(refuse());
        }
    }
    for output in after
        .list_output()
        .filter(|o| o.outpoint.txid == receipt.txid && o.keychain == KeychainKind::External)
    {
        if output.outpoint != receipt {
            // The narrow consumer has one imported receipt. Extra External
            // outputs need their own provenance and are not implicitly imported.
            return Err(refuse());
        }
        let row = data
            .txos
            .iter()
            .find(|t| t.txid == receipt.txid.to_string() && t.vout == receipt.vout)
            .ok_or_else(refuse)?;
        if output.is_spent || output.txout.value.to_sat() == 0 {
            return Err(refuse());
        }
        let mut changed: DbTxoActMod = row.clone().into();
        changed.btc_amount = ActiveValue::Set(output.txout.value.to_sat().to_string());
        changed.exists = ActiveValue::Set(true);
        changed.pending_witness = ActiveValue::Set(false);
        post.update_txo(changed)?;
        post.del_pending_witness_script(output.txout.script_pubkey.to_hex_string())?;
    }
    let actual = post
        .get_txo(&Outpoint {
            txid: receipt.txid.to_string(),
            vout: receipt.vout,
        })?
        .ok_or_else(refuse)?;
    if !actual.exists || actual.pending_witness || actual.btc_amount == "0" {
        return Err(refuse());
    }
    let mut changed: DbBatchTransferActMod = batch.clone().into();
    changed.status = ActiveValue::Set(TransferStatus::Settled);
    // Keep the original timestamps. No ambient clock is part of the frozen
    // observation, and status completion does not grant a new authorization.
    post.update_batch_transfer(&mut changed)?;
    Ok(post)
}

pub(super) fn project(
    wallet: &Wallet,
    recipient: &str,
    contract: ContractId,
    receipt: OutPoint,
    units: u64,
    transaction: &BdkTransaction,
    transfer: &ValidTransfer,
    resolver: &PreFetchResolver,
    bdk_post: ChangeSet,
    source: &serde_json::Value,
) -> Result<super::idb_store::WalletSnapshot, Error> {
    if super::incoming_observation::complete_source(wallet)? != *source
        || transfer.contract_id() != contract
        || !super::incoming_history::safe_to_accept(transfer.validation_status())
        || transaction.compute_txid() != receipt.txid
    {
        return Err(refuse());
    }
    let mut post = wallet.snapshot_for_review()?;
    let data = post.db.get_db_data(false)?;
    let batch = super::incoming_selection::select(
        &data,
        recipient,
        &contract.to_string(),
        &receipt,
        units,
    )?;
    let mut stock = super::idb_store::deserialize_stock(
        post.stock_stash_b64.as_deref().ok_or_else(refuse)?,
        post.stock_state_b64.as_deref().ok_or_else(refuse)?,
        post.stock_index_b64.as_deref().ok_or_else(refuse)?,
    )
    .map_err(|_| refuse())?;
    require_uncolored_inputs(&post.db, &stock, transaction)?;
    let detached = BdkWallet::load()
        .load_wallet_no_persist(bdk_post.clone())
        .map_err(|_| refuse())?
        .ok_or_else(refuse)?;
    post.db = project_db(
        &post.db,
        &wallet.bdk_wallet,
        &detached,
        transaction,
        receipt,
        batch,
    )?;
    // Established RGB primitive on a fully detached copy. An error discards
    // only this prospective copy; the active Stock is never borrowed mutably.
    stock
        .accept_transfer(transfer.clone(), resolver)
        .map_err(|_| refuse())?;
    for info in stock.contracts().map_err(InternalError::from)? {
        if info.id != contract
            && stock
                .contract_assignments_for(info.id, [receipt])
                .map_err(InternalError::from)?
                .values()
                .any(|states| !states.is_empty())
        {
            return Err(refuse());
        }
    }
    let assignments = stock
        .contract_assignments_for(contract, [receipt])
        .map_err(InternalError::from)?;
    let mut received = 0u64;
    for states in assignments.values() {
        for (opout, state) in states {
            if opout.ty != OS_ASSET {
                return Err(refuse());
            }
            let AllocatedState::Amount(value) = state else {
                return Err(refuse());
            };
            received = received.checked_add(value.as_u64()).ok_or_else(refuse)?;
        }
    }
    if received != units {
        return Err(refuse());
    }
    let (stash, state, index) = super::idb_store::serialize_stock(&stock).ok_or_else(refuse)?;
    post.stock_stash_b64 = Some(stash);
    post.stock_state_b64 = Some(state);
    post.stock_index_b64 = Some(index);
    post.bdk_changeset = Some(bdk_post);
    if post.received_consignments.remove(recipient).is_none()
        || super::incoming_observation::complete_source(wallet)? != *source
    {
        return Err(refuse());
    }
    Ok(post)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (Wallet, BdkTransaction, BdkWallet, DbBatchTransfer) {
        let (wallet, tx, prevouts, inclusion) = super::super::incoming_bdk::tests::fixture();
        for output in wallet
            .bdk_wallet
            .list_output()
            .filter(|o| o.keychain == KeychainKind::External)
        {
            wallet.database.set_txo(output.into()).unwrap();
        }
        wallet
            .database
            .set_txo(DbTxoActMod {
                txid: ActiveValue::Set(tx.compute_txid().to_string()),
                vout: ActiveValue::Set(0),
                btc_amount: ActiveValue::Set("0".into()),
                pending_witness: ActiveValue::Set(true),
                ..Default::default()
            })
            .unwrap();
        wallet
            .database
            .set_pending_witness_script(DbPendingWitnessScriptActMod {
                script: ActiveValue::Set(tx.output[0].script_pubkey.to_hex_string()),
                ..Default::default()
            })
            .unwrap();
        let id = wallet
            .database
            .set_batch_transfer(DbBatchTransferActMod {
                txid: ActiveValue::Set(Some(tx.compute_txid().to_string())),
                status: ActiveValue::Set(TransferStatus::WaitingConfirmations),
                created_at: ActiveValue::Set(17),
                updated_at: ActiveValue::Set(18),
                min_confirmations: ActiveValue::Set(1),
                ..Default::default()
            })
            .unwrap();
        let batch = wallet
            .database
            .get_db_data(false)
            .unwrap()
            .batch_transfers
            .into_iter()
            .find(|b| b.idx == id)
            .unwrap();
        let changeset =
            super::super::incoming_bdk::project(&wallet, &tx, &prevouts, inclusion).unwrap();
        let after = BdkWallet::load()
            .load_wallet_no_persist(changeset)
            .unwrap()
            .unwrap();
        (wallet, tx, after, batch)
    }

    fn source(wallet: &Wallet) -> serde_json::Value {
        super::super::incoming_observation::complete_source(wallet).unwrap()
    }

    #[test]
    fn valid_nia_transfer_projects_full_checkpoint_without_active_mutation() {
        use super::super::rust_only::{AssetColoringInfo, ColoringInfo};
        let (mut wallet, mut tx, prevouts, inclusion) =
            super::super::incoming_bdk::tests::fixture();
        let sender = super::super::offline::address_reuse_tests::make_test_wallet(false);
        let mut stock = Stock::in_memory();
        let foreign = tx.input[1].previous_output;
        let issued = ContractBuilder::with(
            Identity::default(),
            NonInflatableAsset::schema(),
            NonInflatableAsset::types(),
            NonInflatableAsset::scripts(),
            wallet.chain_net(),
        )
        .add_global_state(
            "spec",
            AssetSpec {
                ticker: Ticker::try_from("TEST".to_string()).unwrap(),
                name: Name::try_from("Fake projection test".to_string()).unwrap(),
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
        .add_global_state("issuedSupply", Amount::from(1107u64))
        .unwrap()
        .add_fungible_state(
            "assetOwner",
            BlindSeal::new_random(
                RgbTxid::from_str(&foreign.txid.to_string()).unwrap(),
                foreign.vout,
            ),
            1107u64,
        )
        .unwrap()
        .issue_contract()
        .unwrap();
        let contract = issued.contract_id();
        stock.import_contract(issued, &DumbResolver).unwrap();
        *sender.rgb_stock.borrow_mut() = Some(stock);
        // OpretFirst requires OP_RETURN before the wallet P2TR outputs.
        tx.output.insert(
            0,
            TxOut {
                value: BdkAmount::ZERO,
                script_pubkey: ScriptBuf::new_op_return([]),
            },
        );
        let mut psbt = Psbt::from_unsigned_tx(tx).unwrap();
        let transfer = sender
            .color_psbt_and_export(
                &mut psbt,
                ColoringInfo {
                    asset_info_map: HashMap::from([(
                        contract,
                        AssetColoringInfo {
                            output_map: HashMap::from([(1, 1107)]),
                            static_blinding: Some(23),
                        },
                    )]),
                    static_blinding: Some(29),
                    nonce: Some(31),
                },
            )
            .unwrap()
            .remove(0);
        tx = psbt.unsigned_tx;
        let receipt = OutPoint::new(tx.compute_txid(), 1);
        let witness = RgbTxid::from_str(&receipt.txid.to_string()).unwrap();
        let resolver = PreFetchResolver::new(
            HashMap::from([(
                witness,
                WitnessStatus::Resolved(
                    tx.clone(),
                    WitnessOrd::Mined(
                        WitnessPos::bitcoin(NonZeroU32::new(20).unwrap(), 1_700_000_000).unwrap(),
                    ),
                ),
            )]),
            wallet.chain_net(),
        );
        let valid = transfer
            .clone()
            .validate(
                &resolver,
                &ValidationConfig {
                    chain_net: wallet.chain_net(),
                    trusted_typesystem: AssetSchema::Nia.types(),
                    ..Default::default()
                },
            )
            .expect("established RGB validator accepts actual NIA transfer");
        assert!(super::super::incoming_history::safe_to_accept(
            valid.validation_status()
        ));
        let recipient_string = XChainNet::with(
            wallet.chain_net(),
            Beneficiary::WitnessVout(
                Pay2Vout::new(
                    AddressPayload::from_script(&tx.output[receipt.vout as usize].script_pubkey)
                        .unwrap(),
                ),
                None,
            ),
        )
        .to_string();
        let recipient = recipient_string.as_str();
        wallet
            .database
            .set_pending_witness_script(DbPendingWitnessScriptActMod {
                script: ActiveValue::Set(
                    tx.output[receipt.vout as usize]
                        .script_pubkey
                        .to_hex_string(),
                ),
                ..Default::default()
            })
            .unwrap();
        wallet
            .database
            .set_asset(DbAssetActMod {
                id: ActiveValue::Set(contract.to_string()),
                schema: ActiveValue::Set(AssetSchema::Nia),
                ..Default::default()
            })
            .unwrap();
        let mut bytes = vec![];
        transfer.save(&mut bytes).unwrap();
        wallet.received_consignments.insert(recipient.into(), bytes);
        for output in wallet
            .bdk_wallet
            .list_output()
            .filter(|o| o.keychain == KeychainKind::External)
        {
            wallet.database.set_txo(output.into()).unwrap();
        }
        let txo = wallet
            .database
            .set_txo(DbTxoActMod {
                txid: ActiveValue::Set(receipt.txid.to_string()),
                vout: ActiveValue::Set(receipt.vout),
                btc_amount: ActiveValue::Set("0".into()),
                pending_witness: ActiveValue::Set(true),
                ..Default::default()
            })
            .unwrap();
        let batch = wallet
            .database
            .set_batch_transfer(DbBatchTransferActMod {
                txid: ActiveValue::Set(Some(receipt.txid.to_string())),
                status: ActiveValue::Set(TransferStatus::WaitingConfirmations),
                min_confirmations: ActiveValue::Set(1),
                ..Default::default()
            })
            .unwrap();
        let asset = wallet
            .database
            .set_asset_transfer(DbAssetTransferActMod {
                batch_transfer_idx: ActiveValue::Set(batch),
                user_driven: ActiveValue::Set(true),
                asset_id: ActiveValue::Set(Some(contract.to_string())),
                ..Default::default()
            })
            .unwrap();
        wallet
            .database
            .set_transfer(DbTransferActMod {
                asset_transfer_idx: ActiveValue::Set(asset),
                incoming: ActiveValue::Set(true),
                recipient_id: ActiveValue::Set(Some(recipient.into())),
                recipient_type: ActiveValue::Set(Some(RecipientTypeFull::Witness {
                    vout: Some(receipt.vout),
                })),
                ..Default::default()
            })
            .unwrap();
        wallet
            .database
            .set_coloring(DbColoringActMod {
                txo_idx: ActiveValue::Set(txo),
                asset_transfer_idx: ActiveValue::Set(asset),
                r#type: ActiveValue::Set(ColoringType::Receive),
                assignment: ActiveValue::Set(Assignment::Fungible(1107)),
                ..Default::default()
            })
            .unwrap();
        wallet
            .require_exact_pending_witness_receive(
                recipient,
                contract,
                receipt,
                1107,
                wallet.received_consignments.get(recipient).unwrap(),
                &tx.output[receipt.vout as usize].script_pubkey,
            )
            .unwrap();
        let original = source(&wallet);
        let bdk = super::super::incoming_bdk::project(&wallet, &tx, &prevouts, inclusion).unwrap();
        assert!(
            project(
                &wallet,
                recipient,
                contract,
                receipt,
                1108,
                &tx,
                &valid,
                &resolver,
                bdk.clone(),
                &original
            )
            .is_err()
        );
        assert_eq!(
            source(&wallet),
            original,
            "wrong amount cannot mutate active state"
        );
        let post = project(
            &wallet, recipient, contract, receipt, 1107, &tx, &valid, &resolver, bdk, &original,
        )
        .unwrap();
        assert_eq!(
            source(&wallet),
            original,
            "complete active snapshot AND staged BDK unchanged"
        );
        let received_stock = super::super::idb_store::deserialize_stock(
            post.stock_stash_b64.as_ref().unwrap(),
            post.stock_state_b64.as_ref().unwrap(),
            post.stock_index_b64.as_ref().unwrap(),
        )
        .unwrap();
        let amounts: Vec<_> = received_stock
            .contract_assignments_for(contract, [receipt])
            .unwrap()
            .values()
            .flat_map(|states| states.values())
            .cloned()
            .collect();
        assert_eq!(amounts.len(), 1);
        let AllocatedState::Amount(amount) = &amounts[0] else {
            panic!("fungible receipt expected")
        };
        assert_eq!(amount.as_u64(), 1107);
        assert!(post.received_consignments.is_empty());
        assert!(
            post.db.get_db_data(false).unwrap().batch_transfers[0]
                .status
                .settled()
        );
        assert_eq!(
            post.sequence,
            wallet.snapshot_for_review().unwrap().sequence
        );
        // Independently apply established primitives to this disposable in-memory
        // test wallet. Do not adopt the projected snapshot as the actual result.
        let mut update = Update {
            chain: Some(
                wallet
                    .bdk_wallet
                    .local_chain()
                    .tip()
                    .push(inclusion.block_id)
                    .unwrap(),
            ),
            ..Default::default()
        };
        update.tx_update.txs.push(Arc::new(tx.clone()));
        update.tx_update.anchors.insert((inclusion, receipt.txid));
        wallet.bdk_wallet.apply_update(update).unwrap();
        wallet.bdk_wallet.persist(&mut wallet.bdk_database).unwrap();
        wallet
            .rgb_stock
            .borrow_mut()
            .as_mut()
            .unwrap()
            .accept_transfer(valid, &resolver)
            .unwrap();
        let row = wallet
            .database
            .get_txo(&Outpoint {
                txid: receipt.txid.to_string(),
                vout: receipt.vout,
            })
            .unwrap()
            .unwrap();
        let mut changed: DbTxoActMod = row.into();
        changed.btc_amount = ActiveValue::Set("1000".into());
        changed.exists = ActiveValue::Set(true);
        changed.pending_witness = ActiveValue::Set(false);
        wallet.database.update_txo(changed).unwrap();
        let row = wallet
            .database
            .get_db_data(false)
            .unwrap()
            .batch_transfers
            .remove(0);
        let mut changed: DbBatchTransferActMod = row.into();
        changed.status = ActiveValue::Set(TransferStatus::Settled);
        wallet.database.update_batch_transfer(&mut changed).unwrap();
        wallet
            .database
            .del_pending_witness_script(
                tx.output[receipt.vout as usize]
                    .script_pubkey
                    .to_hex_string(),
            )
            .unwrap();
        wallet.received_consignments.remove(recipient).unwrap();
        assert_eq!(
            serde_json::to_value(wallet.snapshot_for_review().unwrap()).unwrap(),
            serde_json::to_value(post).unwrap(),
            "complete Stock, BDK, DB, sequence, intent, artifacts and metadata match"
        );
    }

    #[test]
    fn detached_db_imports_only_receipt_and_preserves_entire_source_and_counters() {
        let (wallet, tx, after, batch) = fixture();
        let original = source(&wallet);
        let receipt = OutPoint::new(tx.compute_txid(), 0);
        require_uncolored_inputs(
            &wallet.database,
            wallet.rgb_stock.borrow().as_ref().unwrap(),
            &tx,
        )
        .unwrap();
        let post = project_db(
            &wallet.database,
            &wallet.bdk_wallet,
            &after,
            &tx,
            receipt,
            &batch,
        )
        .unwrap();
        let mut expected = serde_json::to_value(wallet.database.as_ref()).unwrap();
        let rows = expected["txos"].as_array_mut().unwrap();
        let target = rows
            .iter_mut()
            .find(|r| r["txid"] == receipt.txid.to_string())
            .unwrap();
        target["btc_amount"] = serde_json::json!("1000");
        target["exists"] = serde_json::json!(true);
        target["pending_witness"] = serde_json::json!(false);
        expected["pending_witness_scripts"] = serde_json::json!([]);
        expected["batch_transfers"][0]["status"] =
            serde_json::to_value(TransferStatus::Settled).unwrap();
        assert_eq!(
            serde_json::to_value(&post).unwrap(),
            expected,
            "all DB fields, metadata, row order and counters compared"
        );
        assert_eq!(
            source(&wallet),
            original,
            "full active snapshot and stage unchanged"
        );
        assert_eq!(
            after
                .list_output()
                .find(|o| o.outpoint == OutPoint::new(receipt.txid, 1))
                .unwrap()
                .keychain,
            KeychainKind::Internal,
            "BTC change retained by detached BDK"
        );
    }

    #[test]
    fn missing_mismatched_or_duplicate_source_txo_never_gets_repaired() {
        let (wallet, tx, after, batch) = fixture();
        let receipt = OutPoint::new(tx.compute_txid(), 0);
        let original = source(&wallet);
        let mut value = serde_json::to_value(wallet.database.as_ref()).unwrap();
        let original_db = value.clone();
        for field in ["spent", "exists", "pending_witness", "btc_amount"] {
            value = original_db.clone();
            value["txos"][0][field] = match field {
                "exists" => serde_json::json!(false),
                "btc_amount" => serde_json::json!("999"),
                _ => serde_json::json!(true),
            };
            let db = serde_json::from_value(value.clone()).unwrap();
            assert!(project_db(&db, &wallet.bdk_wallet, &after, &tx, receipt, &batch).is_err());
        }
        value = original_db.clone();
        value["txos"].as_array_mut().unwrap().remove(0);
        let db = serde_json::from_value(value).unwrap();
        assert!(project_db(&db, &wallet.bdk_wallet, &after, &tx, receipt, &batch).is_err());
        value = original_db;
        let row = value["txos"][0].clone();
        value["txos"].as_array_mut().unwrap().push(row);
        let db = serde_json::from_value(value).unwrap();
        assert!(project_db(&db, &wallet.bdk_wallet, &after, &tx, receipt, &batch).is_err());
        assert_eq!(source(&wallet), original);
    }

    #[test]
    fn funding_with_any_db_coloring_is_refused_without_consumption() {
        let (wallet, tx, _, _) = fixture();
        // Add a locally known RGB obligation on the actual spent funding input.
        let point = tx.input[0].previous_output;
        let idx = wallet
            .database
            .set_txo(DbTxoActMod {
                txid: ActiveValue::Set(point.txid.to_string()),
                vout: ActiveValue::Set(point.vout),
                btc_amount: ActiveValue::Set("10000".into()),
                exists: ActiveValue::Set(true),
                ..Default::default()
            })
            .unwrap();
        wallet
            .database
            .set_coloring(DbColoringActMod {
                txo_idx: ActiveValue::Set(idx),
                asset_transfer_idx: ActiveValue::Set(99),
                r#type: ActiveValue::Set(ColoringType::Receive),
                assignment: ActiveValue::Set(Assignment::Fungible(1)),
                ..Default::default()
            })
            .unwrap();
        let original = source(&wallet);
        assert!(
            require_uncolored_inputs(
                &wallet.database,
                wallet.rgb_stock.borrow().as_ref().unwrap(),
                &tx
            )
            .is_err()
        );
        assert_eq!(source(&wallet), original);
    }
}

#[cfg(test)]
mod output_scope_tests {
    use super::*;
    #[test]
    fn extra_external_output_refuses_without_sweeping_it_into_rgb_db() {
        let (wallet, mut tx, prevouts, inclusion) = super::super::incoming_bdk::tests::fixture();
        for output in wallet
            .bdk_wallet
            .list_output()
            .filter(|o| o.keychain == KeychainKind::External)
        {
            wallet.database.set_txo(output.into()).unwrap();
        }
        tx.output.push(TxOut {
            value: BdkAmount::from_sat(500),
            script_pubkey: tx.output[0].script_pubkey.clone(),
        });
        let receipt = OutPoint::new(tx.compute_txid(), 0);
        wallet
            .database
            .set_txo(DbTxoActMod {
                txid: ActiveValue::Set(receipt.txid.to_string()),
                vout: ActiveValue::Set(0),
                btc_amount: ActiveValue::Set("0".into()),
                pending_witness: ActiveValue::Set(true),
                ..Default::default()
            })
            .unwrap();
        wallet
            .database
            .set_pending_witness_script(DbPendingWitnessScriptActMod {
                script: ActiveValue::Set(tx.output[0].script_pubkey.to_hex_string()),
                ..Default::default()
            })
            .unwrap();
        let changeset =
            super::super::incoming_bdk::project(&wallet, &tx, &prevouts, inclusion).unwrap();
        let after = BdkWallet::load()
            .load_wallet_no_persist(changeset)
            .unwrap()
            .unwrap();
        let batch = DbBatchTransfer {
            idx: 1,
            txid: Some(receipt.txid.to_string()),
            status: TransferStatus::WaitingConfirmations,
            created_at: 0,
            updated_at: 0,
            expiration: None,
            min_confirmations: 1,
        };
        let source = super::super::incoming_observation::complete_source(&wallet).unwrap();
        assert!(
            project_db(
                &wallet.database,
                &wallet.bdk_wallet,
                &after,
                &tx,
                receipt,
                &batch
            )
            .is_err()
        );
        assert_eq!(
            super::super::incoming_observation::complete_source(&wallet).unwrap(),
            source
        );
    }
}
