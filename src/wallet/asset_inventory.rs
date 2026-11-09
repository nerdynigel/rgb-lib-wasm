//! Bounded export of standard RGB evidence for explicitly selected native inputs.
//! This is not remote attestation, current Core availability, wallet consent to
//! spend, or a reservation. Consumers must independently validate all of those.
use super::*;

const MAX_SELECTED_INPUTS: usize = 32;
// Existing dedicated public RGB verifier permits at most1_048_576 hex bytes.
const MAX_CONSIGNMENT_BYTES: usize = 524_288;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
/// Public proof exported only on an explicit caller request. The standard
/// consignment can reveal transaction ancestry; do not publish it automatically.
pub struct AssetInventoryProof {
    /// Response version, currently one.
    pub version: u8,
    /// Fixed scope: this alone never establishes remote ownership or readiness.
    pub scope: String,
    /// Current native observation restricted to the exact requested inputs.
    pub selected: AssetSpendView,
    /// Standard RGB transfer file, encoded as lowercase hex without new crypto.
    pub consignment_hex: String,
}

fn refused() -> Error {
    Error::InvalidConsignment
}

/// Stop serialization at the wire budget, rather than allocating an unbounded
/// consignment and checking its size afterwards.
struct ProofBytes(Vec<u8>);
impl io::Write for ProofBytes {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > MAX_CONSIGNMENT_BYTES.saturating_sub(self.0.len()) {
            return Err(io::Error::new(
                ErrorKind::InvalidData,
                "inventory proof size",
            ));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Wallet {
    /// Export standard NIA history for at most32 explicit, sorted, currently
    /// qualified REGTEST carriers. Reuses native DB/Stock/BDK and pending-original
    /// fences; no shadow state, transition, address, signature or write is made.
    /// The caller must separately exclude its own durable browser reservations.
    pub fn asset_inventory_proof(
        &self,
        asset_id: String,
        outpoints: Vec<String>,
    ) -> Result<AssetInventoryProof, Error> {
        if !(1..=MAX_SELECTED_INPUTS).contains(&outpoints.len()) {
            return Err(refused());
        }
        let mut previous = "";
        let mut outputs = Vec::with_capacity(outpoints.len());
        for text in &outpoints {
            if text.len() > 75 || text.as_str() <= previous {
                return Err(refused());
            }
            let point = OutPoint::from_str(text).map_err(|_| refused())?;
            if point.to_string() != *text {
                return Err(refused());
            }
            outputs.push(ExplicitSeal::with(
                RgbTxid::from_str(&point.txid.to_string()).map_err(|_| refused())?,
                point.vout,
            ));
            previous = text;
        }
        let mut selected = self.asset_spend_view(asset_id.clone())?;
        selected
            .inputs
            .retain(|input| outpoints.binary_search(&input.outpoint).is_ok());
        if selected.inputs.len() != outpoints.len() {
            return Err(refused());
        }
        let (sats, units) = selected
            .inputs
            .iter()
            .try_fold((0u64, 0u64), |(sats, units), i| {
                Ok::<_, Error>((
                    sats.checked_add(i.input_sats.parse().map_err(|_| refused())?)
                        .ok_or_else(refused)?,
                    units
                        .checked_add(i.rgb_units.parse().map_err(|_| refused())?)
                        .ok_or_else(refused)?,
                ))
            })?;
        selected.total_sats = sats.to_string();
        selected.total_units = units.to_string();
        let id = ContractId::from_str(&asset_id).map_err(|_| refused())?;
        let runtime = self.rgb_runtime()?;
        if AssetSchema::get_from_contract_id(id, &runtime)? != AssetSchema::Nia {
            return Err(refused());
        }
        // Stock::transfer is the established non-consuming consignment export.
        // No fascia or hypothetical Bitcoin transaction is constructed.
        let transfer = runtime.transfer(id, outputs, [], None)?;
        let mut bytes = ProofBytes(Vec::new());
        transfer.save(&mut bytes)?;
        Ok(AssetInventoryProof {
            version: 1,
            scope: "standard-rgb-inventory-proof-not-ownership-or-reservation".into(),
            selected,
            consignment_hex: hex::encode(bytes.0),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn proof_output_is_bounded_without_accepting_partial_overflow() {
        let mut output = ProofBytes(Vec::new());
        output.write_all(&vec![0; MAX_CONSIGNMENT_BYTES]).unwrap();
        assert!(output.write_all(&[1]).is_err());
        assert_eq!(output.0.len(), MAX_CONSIGNMENT_BYTES);
        assert!(output.0.iter().all(|b| *b == 0));
    }
}
