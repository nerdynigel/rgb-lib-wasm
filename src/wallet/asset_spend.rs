//! Read-only native qualification for a complete fungible RGB input set.
//! DB display allocations alone are insufficient. Require confirmed owned BDK
//! prevouts, the existing original-operation fence, exact DB/Stock agreement,
//! and no other contract or assignment on any returned seal. No preparation,
//! signature, persistence, address generation or reservation release occurs.
use super::*;

const MAX_INPUTS: usize = 4096;
const MAX_ROWS: usize = 32768;

struct QualifiedCarrier {
    point: OutPoint,
    sats: u64,
    units: u64,
    script: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
/// One native-owned, confirmed and currently eligible fungible RGB carrier.
pub struct AssetSpendInput {
    /// Canonical transaction ID and decimal output index.
    pub outpoint: String,
    /// Exact whole satoshis, never a floating-point JSON amount.
    pub input_sats: String,
    /// Exact positive fungible units of the requested contract.
    pub rgb_units: String,
    /// Exact native BDK-owned Taproot script.
    pub script_hex: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
/// Current public observation, never spending authority or a cached balance.
pub struct AssetSpendView {
    /// Response version, currently one.
    pub version: u8,
    /// Fixed `wallet-rgb-spendability` discriminator.
    pub kind: String,
    /// This implementation admits REGTEST only.
    pub network: String,
    /// Authoritative canonical contract identifier; ticker text is refused.
    pub contract_id: String,
    /// Actual BDK checkpoint height.
    pub tip_height: u32,
    /// Actual BDK checkpoint hash.
    pub tip_hash: String,
    /// Checked sum of returned fungible units as a decimal u64 string.
    pub total_units: String,
    /// Checked sum of returned carrier satoshis as a decimal u64 string.
    pub total_sats: String,
    /// All qualified inputs, ordered by canonical ASCII outpoint text.
    pub inputs: Vec<AssetSpendInput>,
}

fn refused() -> Error {
    Error::InvalidPsbt {
        details: "RGB inputs cannot be verified against current native wallet and contract state"
            .into(),
    }
}

impl Wallet {
    /// Observe eligible complete-set candidates. Every selected input must still
    /// be rechecked at native preparation/signing and against caller-held inputs.
    pub fn asset_spend_view(&self, asset_id: String) -> Result<AssetSpendView, Error> {
        let tip = self.bdk_wallet.latest_checkpoint();
        let genesis =
            bitcoin::blockdata::constants::genesis_block(BdkNetwork::Regtest).block_hash();
        if self.bitcoin_network() != BitcoinNetwork::Regtest
            || tip.get(0).is_none_or(|c| c.hash() != genesis)
        {
            return Err(Error::InvalidRecipientNetwork);
        }
        if asset_id.len() > 192 {
            return Err(refused());
        }
        let requested = ContractId::from_str(&asset_id).map_err(|_| refused())?;
        if requested.to_string() != asset_id {
            return Err(refused());
        }
        let db = self.database.get_db_data(false)?;
        if [
            db.txos.len(),
            db.transfers.len(),
            db.asset_transfers.len(),
            db.batch_transfers.len(),
            db.colorings.len(),
        ]
        .into_iter()
        .try_fold(0usize, usize::checked_add)
        .is_none_or(|n| n > MAX_ROWS)
        {
            return Err(refused());
        }
        let unspents = self.database.get_rgb_allocations(
            self.database.get_unspent_txos(db.txos)?,
            Some(db.colorings),
            Some(db.batch_transfers),
            Some(db.asset_transfers),
            Some(db.transfers),
        )?;
        if unspents.len() > MAX_INPUTS {
            return Err(refused());
        }
        // Reuse the same durable native original fence as ordinary RGB sends.
        // Missing/malformed/ambiguous originals are an error, never free inputs.
        let candidates =
            self.wallet_rgb_candidate_inputs(unspents, &asset_id, &mut HashSet::new())?;
        let bdk = self
            .bdk_wallet
            .list_unspent()
            .take(MAX_INPUTS + 1)
            .collect::<Vec<_>>();
        if bdk.len() > MAX_INPUTS {
            return Err(refused());
        }
        let mut owned = HashMap::new();
        for output in bdk {
            if owned.insert(output.outpoint, output).is_some() {
                return Err(refused());
            }
        }
        let mut eligible = BTreeMap::<String, QualifiedCarrier>::new();
        let mut ids = HashSet::new();
        for row in candidates {
            let point =
                OutPoint::from_str(&row.utxo.outpoint().to_string()).map_err(|_| refused())?;
            let Some(output) = owned.get(&point) else {
                continue;
            };
            let ChainPosition::Confirmed { anchor, .. } = output.chain_position else {
                continue;
            };
            if anchor.block_id.height > tip.height() || output.is_spent {
                return Err(refused());
            }
            let original = self.bdk_wallet.get_tx(point.txid).ok_or_else(refused)?;
            if original.tx_node.tx.is_coinbase()
                && u64::from(tip.height()) - u64::from(anchor.block_id.height) + 1 < 100
            {
                continue;
            }
            let sats = output.txout.value.to_sat();
            if sats == 0
                || output.txout.value > BdkAmount::MAX_MONEY
                || !output.txout.script_pubkey.is_p2tr()
                || row.utxo.btc_amount != sats.to_string()
                || !ids.insert(row.utxo.idx)
            {
                return Err(refused());
            }
            let units = row.rgb_allocations.iter().try_fold(0u64, |sum, a| {
                if a.asset_id.as_deref() != Some(asset_id.as_str()) || !a.status.settled() {
                    return Err(refused());
                }
                let Assignment::Fungible(n) = a.assignment else {
                    return Err(refused());
                };
                sum.checked_add(n).ok_or_else(refused)
            })?;
            if units == 0
                || eligible
                    .insert(
                        point.to_string(),
                        QualifiedCarrier {
                            point,
                            sats,
                            units,
                            script: output.txout.script_pubkey.to_hex_string(),
                        },
                    )
                    .is_some()
            {
                return Err(refused());
            }
        }
        let runtime = self.rgb_runtime()?;
        let contracts = runtime
            .contracts()?
            .into_iter()
            .take(MAX_INPUTS + 1)
            .collect::<Vec<_>>();
        if contracts.len() > MAX_INPUTS || !contracts.iter().any(|c| c.id == requested) {
            return Err(refused());
        }
        let points = eligible.values().map(|r| r.point).collect::<Vec<_>>();
        let selected = points.iter().copied().collect::<HashSet<_>>();
        let mut amounts = HashMap::<OutPoint, u64>::new();
        let mut forbidden = HashSet::new();
        let mut opouts = HashSet::new();
        for contract in contracts {
            for (seal, states) in
                runtime.contract_assignments_for(contract.id, points.iter().copied())?
            {
                let point = seal.to_outpoint();
                if !selected.contains(&point) || states.len() > MAX_ROWS {
                    return Err(refused());
                }
                for (opout, state) in states {
                    if !opouts.insert(opout) || opouts.len() > MAX_ROWS {
                        return Err(refused());
                    }
                    if contract.id == requested
                        && opout.ty == OS_ASSET
                        && let AllocatedState::Amount(amount) = state
                    {
                        let total = amounts.entry(point).or_default();
                        *total = total.checked_add(amount.as_u64()).ok_or_else(refused)?;
                        continue;
                    }
                    forbidden.insert(point);
                }
            }
        }
        let mut total_units = 0u64;
        let mut total_sats = 0u64;
        let mut inputs = Vec::new();
        for QualifiedCarrier {
            point,
            sats,
            units,
            script,
        } in eligible.into_values()
        {
            // Another contract/right on the same seal is never burnt to trade
            // this asset. A displayed amount differing from Stock is unknown.
            if forbidden.contains(&point) {
                continue;
            }
            if amounts.get(&point).copied() != Some(units) {
                return Err(refused());
            }
            total_units = total_units.checked_add(units).ok_or_else(refused)?;
            total_sats = total_sats.checked_add(sats).ok_or_else(refused)?;
            if total_sats > BdkAmount::MAX_MONEY.to_sat() {
                return Err(refused());
            }
            inputs.push(AssetSpendInput {
                outpoint: point.to_string(),
                input_sats: sats.to_string(),
                rgb_units: units.to_string(),
                script_hex: script,
            });
        }
        Ok(AssetSpendView {
            version: 1,
            kind: "wallet-rgb-spendability".into(),
            network: "regtest".into(),
            contract_id: asset_id,
            tip_height: tip.height(),
            tip_hash: tip.hash().to_string(),
            total_units: total_units.to_string(),
            total_sats: total_sats.to_string(),
            inputs,
        })
    }
}

#[cfg(test)]
#[path = "asset_spend_tests.rs"]
mod tests;
