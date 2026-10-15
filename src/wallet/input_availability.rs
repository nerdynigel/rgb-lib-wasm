//! Non-mutating local RGB input checks. These facts are never signing authority.
use super::*;

fn unavailable() -> Error {
    Error::InvalidColoringInfo {
        details: "designated RGB input is not proven locally available".into(),
    }
}

impl Wallet {
    /// Check one designated RGB input against this engine's current database and stock.
    /// `inputs` must be the complete transaction input set (bounded to 1024).
    /// The designated input must contain exactly `amount` fungible units of `contract`;
    /// every other input must have no asset assignments known to this engine.
    ///
    /// Refuses pending/absent/spent outputs, unresolved transfers, inconsistent state,
    /// unsupported assignments and arithmetic overflow. Does not sync, write, create a
    /// transfer, consume state, reserve funds or return a reusable approval capability.
    /// This is local snapshot evidence only: callers must separately authenticate chain
    /// prevouts and signing ownership, retain the SAME engine/session/snapshot and
    /// recheck currentness after awaits and before effects. Foreign RGB state unknown to
    /// this engine cannot be certified absent by this query.
    pub fn require_available_rgb_input(
        &self,
        designated: OutPoint,
        contract: ContractId,
        amount: u64,
        inputs: &[OutPoint],
    ) -> Result<(), Error> {
        if amount == 0
            || inputs.is_empty()
            || inputs.len() > 1024
            || self.btcx_pending_fill_v1.is_some()
        {
            return Err(unavailable());
        }
        let unique: HashSet<_> = inputs.iter().copied().collect();
        if unique.len() != inputs.len() || !unique.contains(&designated) {
            return Err(unavailable());
        }
        // Borrow the stock directly. rgb_runtime() temporarily detaches it and can
        // substitute an empty stock; neither is appropriate in a read-only predicate.
        let stock = self.rgb_stock.try_borrow().map_err(|_| unavailable())?;
        let stock = stock.as_ref().ok_or_else(unavailable)?;
        let db = self.database.get_db_data(false)?;
        let expected_id = contract.to_string();
        for input in inputs {
            let matching: Vec<_> = db
                .txos
                .iter()
                .filter(|txo| txo.txid == input.txid.to_string() && txo.vout == input.vout)
                .collect();
            if matching.len() > 1 || (*input == designated && matching.len() != 1) {
                return Err(unavailable());
            }
            let mut db_amount = 0u64;
            for txo in matching {
                if txo.spent || !txo.exists || txo.pending_witness {
                    return Err(unavailable());
                }
                for transfer in &db.transfers {
                    if let Some(crate::database::enums::RecipientTypeFull::Blind {
                        unblinded_utxo,
                    }) = &transfer.recipient_type
                    {
                        if unblinded_utxo == &txo.outpoint() {
                            let (_, batch) = transfer
                                .related_transfers(&db.asset_transfers, &db.batch_transfers)?;
                            if batch.status.pending() {
                                return Err(unavailable());
                            }
                        }
                    }
                }
                for coloring in db.colorings.iter().filter(|c| c.txo_idx == txo.idx) {
                    let asset = db
                        .asset_transfers
                        .iter()
                        .find(|a| a.idx == coloring.asset_transfer_idx)
                        .ok_or_else(unavailable)?;
                    let batch = db
                        .batch_transfers
                        .iter()
                        .find(|b| b.idx == asset.batch_transfer_idx)
                        .ok_or_else(unavailable)?;
                    if batch.status.failed() {
                        continue;
                    }
                    if !batch.status.settled()
                        || !coloring.incoming()
                        || *input != designated
                        || asset.asset_id.as_deref() != Some(expected_id.as_str())
                    {
                        return Err(unavailable());
                    }
                    match coloring.assignment {
                        Assignment::Fungible(value) if value > 0 => {
                            db_amount = db_amount.checked_add(value).ok_or_else(unavailable)?;
                        }
                        _ => return Err(unavailable()),
                    }
                }
                // Include pending blind reservations using the established engine
                // semantics, passing every table explicitly (no fallback reads).
                let allocations = self.database.get_rgb_allocations(
                    vec![txo.clone()],
                    Some(db.colorings.clone()),
                    Some(db.batch_transfers.clone()),
                    Some(db.asset_transfers.clone()),
                    Some(db.transfers.clone()),
                )?;
                if allocations.iter().any(|a| a.pending_blinded != 0) {
                    return Err(unavailable());
                }
            }
            let mut stock_amount = 0u64;
            let mut found_contract = false;
            for info in stock.contracts().map_err(InternalError::from)? {
                let assignments = stock
                    .contract_assignments_for(info.id, [*input])
                    .map_err(InternalError::from)?;
                if info.id == contract {
                    found_contract = true;
                }
                for states in assignments.values() {
                    for (opout, state) in states {
                        if *input != designated || info.id != contract || opout.ty != OS_ASSET {
                            return Err(unavailable());
                        }
                        match state {
                            AllocatedState::Amount(value) if value.as_u64() > 0 => {
                                stock_amount = stock_amount
                                    .checked_add(value.as_u64())
                                    .ok_or_else(unavailable)?;
                            }
                            _ => return Err(unavailable()),
                        }
                    }
                }
            }
            if !found_contract
                || db_amount != stock_amount
                || (*input == designated && stock_amount != amount)
                || (*input != designated && stock_amount != 0)
            {
                return Err(unavailable());
            }
        }
        Ok(())
    }
}

#[cfg(all(test, feature = "test-fixtures"))]
mod tests {
    use super::*;
    use crate::database::memory_db::{ActiveValue, DbTxoActMod};

    fn fixture() -> (Wallet, OutPoint, ContractId) {
        let mut wallet = super::super::offline::address_reuse_tests::make_test_wallet(false);
        // The bare engine helper starts with detached stock. Initialize test
        // stock before the existing consumer-persistence handoff, as a real
        // restored engine would have it; do not weaken that handoff guard.
        *wallet.rgb_stock.borrow_mut() = Some(Stock::in_memory());
        let tx = crate::bitcoin::Transaction {
            version: crate::bitcoin::transaction::Version::TWO,
            lock_time: crate::bitcoin::absolute::LockTime::ZERO,
            input: vec![crate::bitcoin::TxIn::default()],
            output: vec![TxOut {
                value: BdkAmount::from_sat(1000),
                script_pubkey: ScriptBuf::new(),
            }],
        };
        wallet.fund_wallet_for_tests(tx.clone()).unwrap();
        let asset = wallet
            .issue_asset_nia_in_memory(
                "TEST".into(),
                "Fake input availability test".into(),
                0,
                vec![1107],
            )
            .unwrap();
        let contract = ContractId::from_str(&asset.asset_id).unwrap();
        (wallet, OutPoint::new(tx.compute_txid(), 0), contract)
    }

    #[test]
    fn available_input_checks_real_stock_without_mutation() {
        let (wallet, input, contract) = fixture();
        let before = serde_json::to_vec(&wallet.snapshot_for_review().unwrap()).unwrap();
        wallet
            .require_available_rgb_input(input, contract, 1107, &[input])
            .unwrap();
        assert_eq!(
            before,
            serde_json::to_vec(&wallet.snapshot_for_review().unwrap()).unwrap()
        );
        let clean_other_input = OutPoint::new(input.txid, input.vout + 1);
        wallet
            .require_available_rgb_input(input, contract, 1107, &[input, clean_other_input])
            .unwrap();
        assert!(
            wallet
                .require_available_rgb_input(input, contract, 1106, &[input])
                .is_err()
        );
        assert!(
            wallet
                .require_available_rgb_input(input, contract, 0, &[input])
                .is_err()
        );
        assert!(
            wallet
                .require_available_rgb_input(input, contract, 1107, &[])
                .is_err()
        );
        assert!(
            wallet
                .require_available_rgb_input(input, contract, 1107, &[input, input])
                .is_err()
        );
        assert!(
            wallet
                .require_available_rgb_input(input, contract, 1107, &vec![input; 1025])
                .is_err()
        );
        let (_, _, other_contract) = fixture();
        assert!(
            wallet
                .require_available_rgb_input(input, other_contract, 1107, &[input])
                .is_err()
        );
    }

    #[test]
    fn available_input_refuses_spent_missing_and_pending_outputs() {
        for field in ["spent", "missing", "pending"] {
            let (wallet, input, contract) = fixture();
            let mut txo: DbTxoActMod = wallet
                .database
                .get_txo(&input.into())
                .unwrap()
                .unwrap()
                .into();
            match field {
                "spent" => txo.spent = ActiveValue::Set(true),
                "missing" => txo.exists = ActiveValue::Set(false),
                _ => txo.pending_witness = ActiveValue::Set(true),
            }
            wallet.database.update_txo(txo).unwrap();
            assert!(
                wallet
                    .require_available_rgb_input(input, contract, 1107, &[input])
                    .is_err()
            );
        }
    }

    #[test]
    fn available_input_refuses_unsettled_and_broken_provenance() {
        for status in [
            TransferStatus::Initiated,
            TransferStatus::WaitingCounterparty,
            TransferStatus::WaitingConfirmations,
            TransferStatus::Failed,
        ] {
            let (wallet, input, contract) = fixture();
            let batch = wallet.database.iter_batch_transfers().unwrap()[0].clone();
            let mut batch: crate::database::memory_db::DbBatchTransferActMod = batch.into();
            batch.status = ActiveValue::Set(status);
            wallet.database.update_batch_transfer(&mut batch).unwrap();
            assert!(
                wallet
                    .require_available_rgb_input(input, contract, 1107, &[input])
                    .is_err()
            );
        }
        let (wallet, input, contract) = fixture();
        let batch = wallet.database.iter_batch_transfers().unwrap()[0].clone();
        wallet.database.del_batch_transfer(&batch).unwrap();
        assert!(
            wallet
                .require_available_rgb_input(input, contract, 1107, &[input])
                .is_err()
        );
    }

    #[test]
    fn available_input_refuses_second_real_asset_input_and_database_overflow() {
        use crate::database::memory_db::DbColoringActMod;
        let (wallet, input, contract) = fixture();
        let first = wallet.database.get_txo(&input.into()).unwrap().unwrap();
        let mut unavailable_first: DbTxoActMod = first.clone().into();
        unavailable_first.pending_witness = ActiveValue::Set(true);
        wallet.database.update_txo(unavailable_first).unwrap();
        let mut second: DbTxoActMod = first.clone().into();
        second.idx = ActiveValue::NotSet;
        second.vout = ActiveValue::Set(input.vout + 1);
        wallet.database.set_txo(second).unwrap();
        wallet
            .issue_asset_nia_in_memory("OTHER".into(), "Other fake asset".into(), 0, vec![1])
            .unwrap();
        let mut restored: DbTxoActMod = first.into();
        restored.pending_witness = ActiveValue::Set(false);
        wallet.database.update_txo(restored).unwrap();
        wallet
            .require_available_rgb_input(input, contract, 1107, &[input])
            .unwrap();
        let second = OutPoint::new(input.txid, input.vout + 1);
        assert!(
            wallet
                .require_available_rgb_input(input, contract, 1107, &[input, second])
                .is_err()
        );

        let db = wallet.database.get_db_data(false).unwrap();
        let original = db
            .colorings
            .iter()
            .find(|c| c.assignment == Assignment::Fungible(1107))
            .unwrap();
        let overflow = DbColoringActMod {
            txo_idx: ActiveValue::Set(original.txo_idx),
            asset_transfer_idx: ActiveValue::Set(original.asset_transfer_idx),
            r#type: ActiveValue::Set(original.r#type),
            assignment: ActiveValue::Set(Assignment::Fungible(u64::MAX)),
            ..Default::default()
        };
        wallet.database.set_coloring(overflow).unwrap();
        assert!(
            wallet
                .require_available_rgb_input(input, contract, 1107, &[input])
                .is_err()
        );
    }

    #[test]
    fn available_input_refuses_pending_fill_and_blinded_reservation() {
        use crate::database::enums::RecipientTypeFull;
        use crate::database::memory_db::{
            DbAssetTransferActMod, DbBatchTransferActMod, DbTransferActMod,
        };
        let (mut wallet, input, contract) = fixture();
        wallet.btcx_pending_fill_v1 = Some(super::super::pending_fill::storage_fixture());
        assert!(
            wallet
                .require_available_rgb_input(input, contract, 1107, &[input])
                .is_err()
        );
        wallet.btcx_pending_fill_v1 = None;
        let batch = wallet
            .database
            .set_batch_transfer(DbBatchTransferActMod {
                status: ActiveValue::Set(TransferStatus::WaitingCounterparty),
                created_at: ActiveValue::Set(1000),
                updated_at: ActiveValue::Set(1000),
                min_confirmations: ActiveValue::Set(1),
                ..Default::default()
            })
            .unwrap();
        let asset = wallet
            .database
            .set_asset_transfer(DbAssetTransferActMod {
                batch_transfer_idx: ActiveValue::Set(batch),
                asset_id: ActiveValue::Set(None),
                user_driven: ActiveValue::Set(true),
                ..Default::default()
            })
            .unwrap();
        wallet
            .database
            .set_transfer(DbTransferActMod {
                asset_transfer_idx: ActiveValue::Set(asset),
                incoming: ActiveValue::Set(true),
                recipient_type: ActiveValue::Set(Some(RecipientTypeFull::Blind {
                    unblinded_utxo: input.into(),
                })),
                ..Default::default()
            })
            .unwrap();
        let before = serde_json::to_vec(&wallet.snapshot_for_review().unwrap()).unwrap();
        assert!(
            wallet
                .require_available_rgb_input(input, contract, 1107, &[input])
                .is_err()
        );
        assert_eq!(
            before,
            serde_json::to_vec(&wallet.snapshot_for_review().unwrap()).unwrap()
        );
    }

    #[test]
    fn available_input_refuses_unavailable_or_changed_engine() {
        let (wallet, input, contract) = fixture();
        let (other, _, _) = fixture();
        assert!(
            other
                .require_available_rgb_input(input, contract, 1107, &[input])
                .is_err()
        );
        let stock = wallet.rgb_stock.borrow_mut().take();
        assert!(
            wallet
                .require_available_rgb_input(input, contract, 1107, &[input])
                .is_err()
        );
        assert!(wallet.rgb_stock.borrow().is_none());
        *wallet.rgb_stock.borrow_mut() = stock;
        let borrow = wallet.rgb_stock.borrow_mut();
        assert!(
            wallet
                .require_available_rgb_input(input, contract, 1107, &[input])
                .is_err()
        );
        drop(borrow);
        wallet
            .require_available_rgb_input(input, contract, 1107, &[input])
            .unwrap();
        // Stock alone must not rehabilitate a spent database output.
        let mut txo: DbTxoActMod = wallet
            .database
            .get_txo(&input.into())
            .unwrap()
            .unwrap()
            .into();
        txo.spent = ActiveValue::Set(true);
        wallet.database.update_txo(txo).unwrap();
        assert!(
            wallet
                .require_available_rgb_input(input, contract, 1107, &[input])
                .is_err()
        );
    }
}
