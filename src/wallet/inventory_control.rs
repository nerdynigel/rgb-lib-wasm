//! Standard BIP-322 control of explicit currently qualified REGTEST inputs.
//! No caller message/PSBT/key/path, real transaction, persistence or broadcast.
use super::*;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
/// Optional protocol1.29 challenge. Wallet consent/channel binding is required
/// before entry; this engine independently binds actual selected native state.
pub struct InventoryControlChallenge {
    /// Fixed one.
    pub version: u8,
    /// Fixed inventory-control-only, never spending authority.
    pub purpose: String,
    /// REGTEST only.
    pub network: String,
    /// bitcoin or rgb, homogeneous input sets only.
    pub proof_kind: String,
    /// SHA256 of exact canonical browser origin.
    pub origin_sha256: String,
    /// SHA256 of authenticated session ID.
    pub session_sha256: String,
    /// SHA256 of authenticated owner ID.
    pub owner_sha256: String,
    /// Unpredictable server nonce; replay tracking stays with the challenger.
    pub nonce: String,
    /// Canonical Unix seconds, at most60seconds from this invocation.
    pub expires_at: String,
    /// Canonical configured RGB pair identifier, including for Bitcoin proofs.
    pub contract_id: String,
    /// Exact standard RGB bytes SHA256; all zeroes only for Bitcoin.
    pub consignment_sha256: String,
    /// Current native BDK checkpoint height.
    pub tip_height: u32,
    /// Current native BDK checkpoint hash.
    pub tip_hash: String,
    /// One to32 sorted unique exact eligible native inputs.
    pub inputs: Vec<AssetSpendInput>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
/// Standard simple witness for exactly one contextual input.
pub struct InventoryInputProof {
    /// Exact selected outpoint.
    pub outpoint: String,
    /// Bitcoin consensus witness bytes in lowercase hex: one64byte signature.
    pub witness_hex: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
/// Control observation only. No reservation or approval is created or released.
pub struct InventoryControlProof {
    /// Fixed one.
    pub version: u8,
    /// Fixed bip322-simple-p2tr-default.
    pub scheme: String,
    /// Original exact immutable challenge.
    pub challenge: InventoryControlChallenge,
    /// Same input order and cardinality; no partial successful response.
    pub proofs: Vec<InventoryInputProof>,
}

fn refused() -> Error {
    Error::InvalidPsbt {
        details: "Selected inventory control challenge refused".into(),
    }
}

fn seconds() -> Result<u64, Error> {
    super::safe_bitcoin::unix_seconds().map_err(|_| refused())
}

fn hash_text(text: &str) -> bool {
    text.len() == 64
        && text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn integer(text: &str) -> Result<u64, Error> {
    if text.is_empty() || text.len() > 20 {
        return Err(refused());
    }
    let value: u64 = text.parse().map_err(|_| refused())?;
    if value.to_string() != text {
        return Err(refused());
    }
    Ok(value)
}

impl InventoryControlChallenge {
    fn validate(&self, timestamp: u64) -> Result<u64, Error> {
        let expiry = integer(&self.expires_at)?;
        if self.version != 1
            || self.purpose != "inventory-control-only"
            || self.network != "regtest"
            || !matches!(self.proof_kind.as_str(), "bitcoin" | "rgb")
            || !(1..=32).contains(&self.inputs.len())
            || self.tip_height == 0
            || self.contract_id.len() > 192
            || !ContractId::from_str(&self.contract_id)
                .is_ok_and(|c| c.to_string() == self.contract_id)
            || expiry <= timestamp
            || expiry > timestamp.checked_add(60).ok_or_else(refused)?
            || [
                &self.origin_sha256,
                &self.session_sha256,
                &self.owner_sha256,
                &self.nonce,
                &self.consignment_sha256,
                &self.tip_hash,
            ]
            .iter()
            .any(|s| !hash_text(s))
            || (self.proof_kind == "bitcoin") != (self.consignment_sha256 == "0".repeat(64))
        {
            return Err(refused());
        }
        let (mut sats, mut units) = (0u64, 0u64);
        let mut previous = "";
        for input in &self.inputs {
            if input.outpoint.len() > 75 || input.script_hex.len() != 68 {
                return Err(refused());
            }
            let point = OutPoint::from_str(&input.outpoint).map_err(|_| refused())?;
            let script = bitcoin::ScriptBuf::from_hex(&input.script_hex).map_err(|_| refused())?;
            let s = integer(&input.input_sats)?;
            let u = integer(&input.rgb_units)?;
            if point.to_string() != input.outpoint
                || input.outpoint.as_str() <= previous
                || !script.is_p2tr()
                || script.to_hex_string() != input.script_hex
                || s == 0
                || (self.proof_kind == "bitcoin") != (u == 0)
            {
                return Err(refused());
            }
            sats = sats.checked_add(s).ok_or_else(refused)?;
            units = units.checked_add(u).ok_or_else(refused)?;
            if sats > BdkAmount::MAX_MONEY.to_sat() {
                return Err(refused());
            }
            previous = &input.outpoint;
        }
        Ok(expiry)
    }

    /// Protocol-defined message only; callers cannot supply arbitrary text or
    /// a transaction. Context hashes must still bind the actual wallet channel.
    pub fn message(&self, input: &AssetSpendInput) -> String {
        format!(
            "urn:asm:wallet-inventory-control:v1\nPurpose: selected input control only; no transaction or spending permission\nNetwork: regtest\nKind: {}\nOrigin SHA256: {}\nSession SHA256: {}\nOwner SHA256: {}\nNonce: {}\nExpires: {}\nContract: {}\nTip: {}:{}\nConsignment SHA256: {}\nOutpoint: {}\nScript: {}\nSatoshis: {}\nRGB units: {}",
            self.proof_kind,
            self.origin_sha256,
            self.session_sha256,
            self.owner_sha256,
            self.nonce,
            self.expires_at,
            self.contract_id,
            self.tip_height,
            self.tip_hash,
            self.consignment_sha256,
            input.outpoint,
            input.script_hex,
            input.input_sats,
            input.rgb_units,
        )
    }
}

impl Wallet {
    /// Prove control with standard BIP-322 synthetic message transactions.
    /// Rechecks qualified DB/Stock/BDK/native holds and exact current evidence.
    /// Browser/session consent and its shared holds must be checked by the caller
    /// under its existing exclusive reservation lock. Never spends or persists.
    pub fn prove_inventory_control(
        &self,
        challenge: InventoryControlChallenge,
    ) -> Result<InventoryControlProof, Error> {
        let timestamp = seconds()?;
        let started = instant::Instant::now();
        let expires = challenge.validate(timestamp)?;
        let fresh = || -> Result<(), Error> {
            let current = seconds()?;
            if current < timestamp || current >= expires || started.elapsed().as_secs() >= 60 {
                return Err(refused());
            }
            Ok(())
        };
        if self.bitcoin_network() != BitcoinNetwork::Regtest {
            return Err(Error::InvalidRecipientNetwork);
        }
        let (height, hash, selected) = if challenge.proof_kind == "rgb" {
            let proof = self.asset_inventory_proof(
                challenge.contract_id.clone(),
                challenge
                    .inputs
                    .iter()
                    .map(|i| i.outpoint.clone())
                    .collect(),
            )?;
            let bytes = hex::decode(proof.consignment_hex).map_err(|_| refused())?;
            if <sha256::Hash as bitcoin::hashes::Hash>::hash(&bytes).to_string()
                != challenge.consignment_sha256
            {
                return Err(refused());
            }
            (
                proof.selected.tip_height,
                proof.selected.tip_hash,
                proof.selected.inputs,
            )
        } else {
            let view = self.bitcoin_spend_view()?;
            let selected = challenge
                .inputs
                .iter()
                .map(|requested| {
                    let actual = view
                        .inputs
                        .iter()
                        .find(|i| i.outpoint == requested.outpoint)
                        .ok_or_else(refused)?;
                    Ok(AssetSpendInput {
                        outpoint: actual.outpoint.clone(),
                        input_sats: actual.amount_sats.clone(),
                        rgb_units: "0".into(),
                        script_hex: actual.script_hex.clone(),
                    })
                })
                .collect::<Result<Vec<_>, Error>>()?;
            (view.tip_height, view.tip_hash, selected)
        };
        if height != challenge.tip_height
            || hash != challenge.tip_hash
            || selected != challenge.inputs
        {
            return Err(refused());
        }
        let mut proofs = Vec::with_capacity(selected.len());
        for input in selected {
            fresh()?;
            let script = bitcoin::ScriptBuf::from_hex(&input.script_hex).map_err(|_| refused())?;
            let address =
                BdkAddress::from_script(&script, BdkNetwork::Regtest).map_err(|_| refused())?;
            let message = challenge.message(&input);
            let to_spend = bip322::create_to_spend(&address, &message).map_err(|_| refused())?;
            let mut psbt = bip322::create_to_sign(&to_spend, None).map_err(|_| refused())?;
            psbt.inputs[0].sighash_type = Some(bitcoin::sighash::TapSighashType::Default.into());
            self._sign_psbt(&mut psbt, None).map_err(|_| refused())?;
            let witness = psbt.inputs[0]
                .final_script_witness
                .take()
                .ok_or_else(refused)?;
            if witness.len() != 1 || witness.iter().next().is_none_or(|s| s.len() != 64) {
                return Err(refused());
            }
            bip322::verify_simple(&address, &message, witness.clone()).map_err(|_| refused())?;
            proofs.push(InventoryInputProof {
                outpoint: input.outpoint,
                witness_hex: hex::encode(bitcoin::consensus::serialize(&witness)),
            });
        }
        fresh()?;
        Ok(InventoryControlProof {
            version: 1,
            scheme: "bip322-simple-p2tr-default".into(),
            challenge,
            proofs,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn messages_match_exact_protocol_129_bitcoin_and_rgb_vectors() {
        let cases: serde_json::Value =
            serde_json::from_str(include_str!("testdata/wallet-inventory-control.json")).unwrap();
        for case in cases["vectors"].as_array().unwrap() {
            let c: InventoryControlChallenge =
                serde_json::from_value(case["challenge"].clone()).unwrap();
            let t = integer(&c.expires_at).unwrap() - 30;
            c.validate(t).unwrap();
            assert_eq!(c.message(&c.inputs[0]), case["message"].as_str().unwrap());
        }
    }
}
