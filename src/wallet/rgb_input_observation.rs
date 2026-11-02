//! Read-only comparison of actual Stock, database allocations and BDK inputs.
//! This is diagnostic evidence, never a spend permit, balance override, original
//! reconstruction, preparation retry or reservation release.
use super::*;

impl Wallet {
    /// Observe current REGTEST RGB input state without creating an address or
    /// transaction, changing Stock, signing, persisting, or accessing the network.
    /// Integer amounts are decimal strings. Missing DB allocations and missing
    /// Stock assignments are both visible rather than interpreted as permission.
    pub fn rgb_input_observation(&self, asset_id: String) -> Result<serde_json::Value, Error> {
        if self.bitcoin_network() != BitcoinNetwork::Regtest || asset_id.len() > 192 {
            return Err(Error::InvalidRecipientNetwork);
        }
        let contract = ContractId::from_str(&asset_id).map_err(|_| Error::InvalidRecipientMap)?;
        if contract.to_string() != asset_id {
            return Err(Error::InvalidRecipientMap);
        }
        let data = self.database.get_db_data(false)?;
        if [
            data.txos.len(),
            data.transfers.len(),
            data.asset_transfers.len(),
            data.batch_transfers.len(),
            data.colorings.len(),
        ]
        .into_iter()
        .try_fold(0usize, usize::checked_add)
        .is_none_or(|n| n > 32768)
        {
            return Err(Error::InvalidRecipientMap);
        }
        let utxos = self.database.get_unspent_txos(data.txos)?;
        let unspents = self.database.get_rgb_allocations(
            utxos,
            Some(data.colorings),
            Some(data.batch_transfers),
            Some(data.asset_transfers),
            Some(data.transfers),
        )?;
        let bdk = self
            .bdk_wallet
            .list_unspent()
            .take(4097)
            .collect::<Vec<_>>();
        let bdk_points = bdk.iter().map(|u| u.outpoint).collect::<HashSet<_>>();
        let mut points = bdk_points.clone();
        let mut database_inputs = HashMap::new();
        for row in &unspents {
            let point = OutPoint::from_str(&row.utxo.outpoint().to_string())
                .map_err(|_| Error::InvalidAssignment)?;
            if database_inputs.insert(point, row).is_some() {
                return Err(Error::InvalidAssignment);
            }
            points.insert(point);
        }
        if bdk.len() > 4096 || unspents.len() > 4096 || points.len() > 4096 {
            return Err(Error::InvalidRecipientMap);
        }
        let held = self.managed_rgb_input_holds(None)?;
        let runtime = self.rgb_runtime()?;
        let mut stock_units = HashMap::<OutPoint, u64>::new();
        let mut other_state = HashSet::new();
        let mut opouts = HashSet::new();
        for (seal, states) in runtime.contract_assignments_for(contract, points.iter().copied())? {
            let point = seal.to_outpoint();
            if !points.contains(&point) || states.len() > 32768 {
                return Err(Error::InvalidAssignment);
            }
            for (opout, state) in states {
                if !opouts.insert(opout) || opouts.len() > 32768 {
                    return Err(Error::InvalidAssignment);
                }
                if let AllocatedState::Amount(amount) = state
                    && opout.ty == OS_ASSET
                {
                    let n = stock_units.entry(point).or_default();
                    *n = n
                        .checked_add(amount.as_u64())
                        .ok_or(Error::InvalidAssignment)?;
                } else {
                    other_state.insert(point);
                }
            }
        }
        let mut points = points.into_iter().collect::<Vec<_>>();
        points.sort();
        let mut rows = Vec::new();
        for point in points {
            let db = database_inputs.get(&point);
            let mut settled = 0u64;
            let mut pending = 0u64;
            let mut non_fungible = false;
            if let Some(db) = db {
                for allocation in &db.rgb_allocations {
                    if allocation.asset_id.as_deref() != Some(&asset_id) {
                        continue;
                    }
                    match allocation.assignment {
                        Assignment::Fungible(n) => {
                            if allocation.status.settled() {
                                settled = settled.checked_add(n).ok_or(Error::InvalidAssignment)?;
                            } else if allocation.status.pending() {
                                pending = pending.checked_add(n).ok_or(Error::InvalidAssignment)?;
                            }
                        }
                        _ => non_fungible = true,
                    }
                }
            }
            let units = stock_units.get(&point).copied().unwrap_or(0);
            if units == 0
                && settled == 0
                && pending == 0
                && !other_state.contains(&point)
                && !non_fungible
            {
                continue;
            }
            rows.push(serde_json::json!({
                "outpoint": point.to_string(),
                "stock_rgb_units": units.to_string(),
                "stock_other_assignment": other_state.contains(&point),
                "database_settled_rgb_units": settled.to_string(),
                "database_pending_rgb_units": pending.to_string(),
                "database_other_assignment": non_fungible,
                "database_exists": db.map(|u| u.utxo.exists),
                "database_pending_blinded": db.map(|u| u.pending_blinded),
                "bdk_unspent": bdk_points.contains(&point),
                "native_managed_hold": held.contains(&point)
            }));
        }
        Ok(serde_json::json!({
            "version": 1,
            "kind": "wallet-rgb-input-observation",
            "network": "regtest",
            "contract_id": asset_id,
            "tip_height": self.bdk_wallet.latest_checkpoint().height(),
            "online_configured": self.online_data.is_some(),
            "spending_authority": false,
            "rows": rows
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::super::safe_bitcoin::tests::{assign_stock, coin, wallet};
    use super::*;

    #[test]
    fn actual_stock_without_display_allocations_is_visible_and_observation_does_not_write() {
        let mut wallet = wallet();
        let point = coin(&mut wallet, KeychainKind::External, 1000, true, false);
        assign_stock(&wallet, point, false);
        let contract = wallet
            .rgb_runtime()
            .unwrap()
            .contracts()
            .unwrap()
            .into_iter()
            .next()
            .unwrap()
            .id;
        let before_db = serde_json::to_vec(wallet.database.as_ref()).unwrap();
        let before_stock = super::super::super::idb_store::serialize_stock(
            wallet.rgb_stock.borrow().as_ref().unwrap(),
        )
        .unwrap();
        let sequence = wallet
            .idb_sequence
            .load(std::sync::atomic::Ordering::SeqCst);
        let observed = wallet.rgb_input_observation(contract.to_string()).unwrap();
        assert_eq!(observed["spending_authority"], false);
        assert_eq!(observed["rows"].as_array().unwrap().len(), 1);
        assert_eq!(observed["rows"][0]["outpoint"], point.to_string());
        assert_eq!(observed["rows"][0]["stock_rgb_units"], "3");
        assert_eq!(observed["rows"][0]["database_settled_rgb_units"], "0");
        assert_eq!(observed["rows"][0]["bdk_unspent"], true);
        assert_eq!(
            serde_json::to_vec(wallet.database.as_ref()).unwrap(),
            before_db
        );
        assert_eq!(
            super::super::super::idb_store::serialize_stock(
                wallet.rgb_stock.borrow().as_ref().unwrap()
            )
            .unwrap(),
            before_stock
        );
        assert_eq!(
            wallet
                .idb_sequence
                .load(std::sync::atomic::Ordering::SeqCst),
            sequence
        );
    }

    #[test]
    fn non_fungible_rights_are_not_reported_as_spendable_asset_units() {
        let mut wallet = wallet();
        let point = coin(&mut wallet, KeychainKind::External, 1000, true, false);
        assign_stock(&wallet, point, true);
        let contract = wallet
            .rgb_runtime()
            .unwrap()
            .contracts()
            .unwrap()
            .into_iter()
            .next()
            .unwrap()
            .id;
        let observed = wallet.rgb_input_observation(contract.to_string()).unwrap();
        assert_eq!(observed["rows"][0]["stock_rgb_units"], "0");
        assert_eq!(observed["rows"][0]["stock_other_assignment"], true);
        assert_eq!(observed["spending_authority"], false);
    }

    #[test]
    fn malformed_or_ticker_identity_is_refused_without_state_change() {
        let wallet = wallet();
        for asset in ["USDT", "rgb:invalid", "", &"x".repeat(193)] {
            assert!(wallet.rgb_input_observation(asset.into()).is_err());
        }
        assert!(wallet.transfer_artifacts.is_empty());
        assert_eq!(
            wallet
                .idb_sequence
                .load(std::sync::atomic::Ordering::SeqCst),
            0
        );
    }
}
