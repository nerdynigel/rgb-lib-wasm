//! Read-only selection for the exact incoming NIA confirmation consumer.
//! Selection is local evidence only, never chain proof or mutation authority.
use super::*;
use crate::database::DbData;

fn refuse() -> Error {
    Error::InvalidConsignment
}

fn one<T>(mut rows: impl Iterator<Item = T>) -> Result<T, Error> {
    let row = rows.next().ok_or_else(refuse)?;
    if rows.next().is_some() {
        return Err(refuse());
    }
    Ok(row)
}

fn unique_indices(indices: impl Iterator<Item = i32>) -> Result<(), Error> {
    let mut seen = HashSet::new();
    for index in indices {
        if index <= 0 || !seen.insert(index) {
            return Err(refuse());
        }
    }
    Ok(())
}

/// Exact DB relationship checks, with no lookup that panics on corrupt rows.
pub(super) fn select<'a>(
    db: &'a DbData,
    recipient: &str,
    contract: &str,
    outpoint: &OutPoint,
    units: u64,
) -> Result<&'a DbBatchTransfer, Error> {
    if recipient.is_empty() || units == 0 {
        return Err(refuse());
    }
    unique_indices(db.batch_transfers.iter().map(|r| r.idx))?;
    unique_indices(db.asset_transfers.iter().map(|r| r.idx))?;
    unique_indices(db.transfers.iter().map(|r| r.idx))?;
    unique_indices(db.colorings.iter().map(|r| r.idx))?;
    unique_indices(db.txos.iter().map(|r| r.idx))?;
    // Refuse dangling relationships even outside the chosen receipt. Such a
    // source cannot establish that the preparation has isolated all obligations.
    for asset in &db.asset_transfers {
        one(db
            .batch_transfers
            .iter()
            .filter(|b| b.idx == asset.batch_transfer_idx))?;
    }
    for transfer in &db.transfers {
        one(db
            .asset_transfers
            .iter()
            .filter(|a| a.idx == transfer.asset_transfer_idx))?;
    }
    for coloring in &db.colorings {
        one(db
            .asset_transfers
            .iter()
            .filter(|a| a.idx == coloring.asset_transfer_idx))?;
        one(db.txos.iter().filter(|t| t.idx == coloring.txo_idx))?;
    }
    let transfer = one(db
        .transfers
        .iter()
        .filter(|t| t.recipient_id.as_deref() == Some(recipient)))?;
    if !transfer.incoming
        || transfer.recipient_type
            != Some(RecipientTypeFull::Witness {
                vout: Some(outpoint.vout),
            })
        || transfer.ack == Some(false)
        || !matches!(transfer.requested_assignment, None | Some(Assignment::Any))
            && transfer.requested_assignment != Some(Assignment::Fungible(units))
    {
        return Err(refuse());
    }
    let asset = one(db
        .asset_transfers
        .iter()
        .filter(|a| a.idx == transfer.asset_transfer_idx))?;
    if asset.asset_id.as_deref() != Some(contract) || !asset.user_driven {
        return Err(refuse());
    }
    let batch = one(db
        .batch_transfers
        .iter()
        .filter(|b| b.idx == asset.batch_transfer_idx))?;
    let txid = outpoint.txid.to_string();
    if !batch.waiting_confirmations()
        || batch.txid.as_deref() != Some(txid.as_str())
        || batch.min_confirmations == 0
    {
        return Err(refuse());
    }
    // Exactly one transfer/asset in the selected batch; no second pending
    // operation may be swept into confirmation or unrelated sync effects.
    one(db
        .asset_transfers
        .iter()
        .filter(|a| a.batch_transfer_idx == batch.idx))?;
    one(db
        .transfers
        .iter()
        .filter(|t| t.asset_transfer_idx == asset.idx))?;
    if db
        .batch_transfers
        .iter()
        .any(|b| b.idx != batch.idx && b.pending())
    {
        return Err(refuse());
    }
    let txo = one(db
        .txos
        .iter()
        .filter(|t| t.txid == txid && t.vout == outpoint.vout))?;
    if txo.spent || txo.exists || !txo.pending_witness || txo.btc_amount != "0" {
        return Err(refuse());
    }
    let mut received = 0u64;
    for coloring in db
        .colorings
        .iter()
        .filter(|c| c.asset_transfer_idx == asset.idx || c.txo_idx == txo.idx)
    {
        if coloring.asset_transfer_idx != asset.idx
            || coloring.txo_idx != txo.idx
            || coloring.r#type != ColoringType::Receive
        {
            return Err(refuse());
        }
        let Assignment::Fungible(amount) = coloring.assignment else {
            return Err(refuse());
        };
        if amount == 0 {
            return Err(refuse());
        }
        received = received.checked_add(amount).ok_or_else(refuse)?;
    }
    if received != units {
        return Err(refuse());
    }
    Ok(batch)
}

impl Wallet {
    /// Check one exact imported, unconfirmed NIA witness receipt against local
    /// DB, consignment bytes and indexed script ownership, without mutation,
    /// network access, sequence allocation or persistence. The caller supplies
    /// independently authenticated terms. This does NOT validate consignment
    /// allocations/history, confirmations, prospective post-state or consent;
    /// it cannot authorize settlement and returns no consuming capability.
    pub fn require_exact_pending_witness_receive(
        &self,
        recipient: &str,
        contract: ContractId,
        outpoint: OutPoint,
        units: u64,
        consignment: &[u8],
        script: &ScriptBuf,
    ) -> Result<(), Error> {
        if consignment.is_empty()
            || consignment.len() > 8_000_000
            || self.btcx_pending_fill_v1.is_some()
            || !self.transfer_artifacts.is_empty()
            || self.received_consignments.len() != 1
            || self.received_consignments.get(recipient).map(Vec::as_slice) != Some(consignment)
            || !self.is_local_script(script)
        {
            return Err(refuse());
        }
        let scripts = self.database.iter_pending_witness_scripts()?;
        if scripts.len() != 1 {
            return Err(refuse());
        }
        let payload = AddressPayload::from_script(script).map_err(|_| refuse())?;
        let beneficiary = Beneficiary::WitnessVout(Pay2Vout::new(payload), None);
        if XChainNet::with(self.chain_net(), beneficiary).to_string() != recipient {
            return Err(refuse());
        }
        let pending_script = one(scripts
            .iter()
            .filter(|s| s.script == script.to_hex_string()))?;
        if pending_script.idx <= 0 {
            return Err(refuse());
        }
        unique_indices(scripts.iter().map(|s| s.idx))?;
        let db = self.database.get_db_data(false)?;
        select(&db, recipient, &contract.to_string(), &outpoint, units)?;
        let assets = self.database.iter_assets()?;
        unique_indices(assets.iter().map(|a| a.idx))?;
        let asset = one(assets.iter().filter(|a| a.id == contract.to_string()))?;
        if asset.schema != AssetSchema::Nia {
            return Err(refuse());
        }
        let transfer = RgbTransfer::load(consignment).map_err(|_| refuse())?;
        let mut canonical = vec![];
        transfer.save(&mut canonical).map_err(|_| refuse())?;
        if canonical != consignment {
            return Err(refuse());
        }
        if transfer.contract_id() != contract
            || AssetSchema::try_from(transfer.schema_id())? != AssetSchema::Nia
        {
            return Err(refuse());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn outpoint() -> OutPoint {
        OutPoint::new(crate::bitcoin::Txid::from_str(&"01".repeat(32)).unwrap(), 2)
    }
    fn fixture() -> DbData {
        DbData {
            batch_transfers: vec![DbBatchTransfer {
                idx: 1,
                txid: Some(outpoint().txid.to_string()),
                status: TransferStatus::WaitingConfirmations,
                created_at: 0,
                updated_at: 0,
                expiration: None,
                min_confirmations: 1,
            }],
            asset_transfers: vec![DbAssetTransfer {
                idx: 1,
                user_driven: true,
                batch_transfer_idx: 1,
                asset_id: Some("contract".into()),
            }],
            transfers: vec![DbTransfer {
                idx: 1,
                asset_transfer_idx: 1,
                requested_assignment: None,
                incoming: true,
                recipient_type: Some(RecipientTypeFull::Witness { vout: Some(2) }),
                recipient_id: Some("recipient".into()),
                ack: None,
                invoice_string: None,
            }],
            txos: vec![DbTxo {
                idx: 1,
                txid: outpoint().txid.to_string(),
                vout: 2,
                btc_amount: "0".into(),
                spent: false,
                exists: false,
                pending_witness: true,
            }],
            colorings: vec![DbColoring {
                idx: 1,
                txo_idx: 1,
                asset_transfer_idx: 1,
                r#type: ColoringType::Receive,
                assignment: Assignment::Fungible(7),
            }],
        }
    }
    fn check(db: &DbData) -> bool {
        select(db, "recipient", "contract", &outpoint(), 7).is_ok()
    }
    #[test]
    fn exact_receipt_and_checked_assignment_sum() {
        let mut db = fixture();
        assert!(check(&db));
        db.colorings[0].assignment = Assignment::Fungible(3);
        let mut second = db.colorings[0].clone();
        second.idx = 2;
        second.assignment = Assignment::Fungible(4);
        db.colorings.push(second);
        assert!(check(&db));
        db.colorings[0].assignment = Assignment::Fungible(u64::MAX);
        assert!(!check(&db));
    }
    #[test]
    fn identity_status_and_local_placeholder_mismatches_refuse() {
        let changes: Vec<fn(&mut DbData)> = vec![
            |d| d.transfers[0].recipient_id = Some("foreign".into()),
            |d| d.transfers[0].incoming = false,
            |d| d.transfers[0].recipient_type = Some(RecipientTypeFull::Witness { vout: Some(3) }),
            |d| d.transfers[0].ack = Some(false),
            |d| d.transfers[0].requested_assignment = Some(Assignment::Fungible(8)),
            |d| d.transfers[0].requested_assignment = Some(Assignment::NonFungible),
            |d| d.asset_transfers[0].asset_id = Some("foreign".into()),
            |d| d.asset_transfers[0].user_driven = false,
            |d| d.batch_transfers[0].txid = None,
            |d| d.batch_transfers[0].status = TransferStatus::Settled,
            |d| d.batch_transfers[0].status = TransferStatus::Failed,
            |d| d.batch_transfers[0].min_confirmations = 0,
            |d| d.txos[0].spent = true,
            |d| d.txos[0].exists = true,
            |d| d.txos[0].pending_witness = false,
            |d| d.txos[0].btc_amount = "1".into(),
            |d| d.colorings[0].assignment = Assignment::Fungible(0),
            |d| d.colorings[0].assignment = Assignment::NonFungible,
            |d| d.colorings[0].r#type = ColoringType::Issue,
        ];
        for (i, change) in changes.iter().enumerate() {
            let mut db = fixture();
            change(&mut db);
            assert!(!check(&db), "case {i}");
        }
    }
    #[test]
    fn duplicate_missing_and_dangling_rows_refuse_without_panics() {
        let changes: Vec<fn(&mut DbData)> = vec![
            |d| d.batch_transfers.push(d.batch_transfers[0].clone()),
            |d| d.asset_transfers.push(d.asset_transfers[0].clone()),
            |d| d.transfers.push(d.transfers[0].clone()),
            |d| d.txos.push(d.txos[0].clone()),
            |d| d.colorings.push(d.colorings[0].clone()),
            |d| d.batch_transfers.clear(),
            |d| d.asset_transfers.clear(),
            |d| d.transfers.clear(),
            |d| d.txos.clear(),
            |d| d.colorings.clear(),
            |d| d.asset_transfers[0].batch_transfer_idx = 99,
            |d| d.transfers[0].asset_transfer_idx = 99,
            |d| d.colorings[0].asset_transfer_idx = 99,
            |d| d.colorings[0].txo_idx = 99,
            |d| d.colorings[0].idx = 0,
        ];
        for (i, change) in changes.iter().enumerate() {
            let mut db = fixture();
            change(&mut db);
            assert!(!check(&db), "case {i}");
        }
    }
    #[test]
    fn unrelated_pending_or_shared_output_refuses_but_terminal_history_survives() {
        for status in [
            TransferStatus::Initiated,
            TransferStatus::WaitingCounterparty,
            TransferStatus::WaitingConfirmations,
            TransferStatus::Settled,
            TransferStatus::Failed,
        ] {
            let mut db = fixture();
            let mut batch = db.batch_transfers[0].clone();
            batch.idx = 2;
            batch.status = status;
            db.batch_transfers.push(batch);
            assert_eq!(check(&db), !status.pending());
        }
        let mut db = fixture();
        let mut coloring = db.colorings[0].clone();
        coloring.idx = 2;
        coloring.asset_transfer_idx = 2;
        db.colorings.push(coloring);
        assert!(!check(&db));
        let mut db = fixture();
        let mut txo = db.txos[0].clone();
        txo.idx = 2;
        db.txos.push(txo);
        assert!(!check(&db));
    }
}

#[cfg(test)]
mod wallet_tests {
    use super::*;
    use strict_encoding::StrictDumb;

    #[test]
    fn real_wallet_refusal_preserves_complete_snapshot_and_bdk_stage() {
        let mut wallet = super::super::offline::address_reuse_tests::make_test_wallet(false);
        *wallet.rgb_stock.borrow_mut() = Some(Stock::in_memory());
        let contract = RgbTransfer::strict_dumb().contract_id();
        let outpoint = OutPoint::new(crate::bitcoin::Txid::from_str(&"01".repeat(32)).unwrap(), 2);
        let script = wallet
            .bdk_wallet
            .reveal_next_address(KeychainKind::External)
            .address
            .script_pubkey();
        let recipient = XChainNet::with(
            wallet.chain_net(),
            Beneficiary::WitnessVout(
                Pay2Vout::new(AddressPayload::from_script(&script).unwrap()),
                None,
            ),
        )
        .to_string();
        wallet
            .received_consignments
            .insert(recipient.clone(), b"invalid".to_vec());
        wallet
            .database
            .set_pending_witness_script(DbPendingWitnessScriptActMod {
                script: ActiveValue::Set(script.to_hex_string()),
                ..Default::default()
            })
            .unwrap();
        let before = serde_json::to_value(wallet.snapshot_for_review().unwrap()).unwrap();
        let stage = serde_json::to_value(wallet.bdk_wallet.staged()).unwrap();
        for (who, output, bytes, local_script) in [
            (
                recipient.as_str(),
                outpoint,
                b"invalid".as_slice(),
                script.clone(),
            ),
            ("foreign", outpoint, b"invalid".as_slice(), script.clone()),
            (
                recipient.as_str(),
                outpoint,
                b"changed".as_slice(),
                script.clone(),
            ),
            (
                recipient.as_str(),
                outpoint,
                b"invalid".as_slice(),
                ScriptBuf::new(),
            ),
        ] {
            assert!(
                wallet
                    .require_exact_pending_witness_receive(
                        who,
                        contract,
                        output,
                        7,
                        bytes,
                        &local_script
                    )
                    .is_err()
            );
            assert_eq!(
                serde_json::to_value(wallet.snapshot_for_review().unwrap()).unwrap(),
                before
            );
            assert_eq!(
                serde_json::to_value(wallet.bdk_wallet.staged()).unwrap(),
                stage
            );
        }
    }
}
