//! Ordinary Bitcoin self-payment preparation. Existing Native self-payment
//! records, signer, one-use broadcast and encrypted recovery remain authoritative.
use super::*;

/// Exact, bounded wallet-local preparation budget. No destination is supplied:
/// Native creates all recipients from the wallet's ordinary Bitcoin descriptor.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BitcoinInputPreparationRequest {
    /// Explicit version, currently one.
    pub version: u8,
    /// Fixed REGTEST network.
    pub network: String,
    /// Stable wallet-local UUIDv4/v7; never silently reused with new terms.
    pub operation_id: String,
    /// Two to eight positive, canonical integer satoshi amounts.
    pub output_sats: Vec<String>,
    /// Canonical whole satoshis per virtual byte, one through1000.
    pub fee_rate: String,
    /// Exact ceiling for this preparation's miner fee, at most10000sats.
    pub maximum_fee_sats: String,
    /// Original preparation/signing deadline, at most120seconds ahead.
    pub expires_at: String,
    /// Additional durable caller-owned reservations, excluded before selection.
    pub excluded_inputs: Vec<String>,
}

#[cfg(test)]
pub(in crate::wallet) mod tests {
    //! Real native BDK graph and signatures; invented local coins, no network,
    //! funding, broadcaster or atomic settlement claim.
    use super::*;
    use crate::wallet::online::safe_bitcoin::tests::{coin, wallet as new_wallet};

    pub(in crate::wallet) fn fixture() -> (Wallet, BitcoinInputPreparationRequest, String) {
        let mut wallet = new_wallet();
        coin(&mut wallet, KeychainKind::Internal, 100_000, true, false);
        let request = BitcoinInputPreparationRequest {
            version: 1,
            network: "regtest".into(),
            operation_id: "01a19805-0462-759b-9050-b9b0feb9da8f".into(),
            output_sats: vec!["4000".into(); 3],
            fee_rate: "1".into(),
            maximum_fee_sats: "1000".into(),
            expires_at: (super::super::super::safe_bitcoin::unix_seconds().unwrap() + 120)
                .to_string(),
            excluded_inputs: vec![],
        };
        let data = wallet.build_bitcoin_inputs(request.clone()).unwrap();
        let txid = data["txid"].as_str().unwrap().to_owned();
        (wallet, request, txid)
    }

    #[test]
    fn exact_original_input_view_is_readonly_keeps_holds_and_excludes_other_free_coins() {
        let (mut wallet, request, txid) = fixture();
        coin(&mut wallet, KeychainKind::Internal, 20_000, true, false);
        assert_eq!(wallet.bitcoin_spend_view().unwrap().total_sats, "20000");
        let before = serde_json::to_value(&wallet.transfer_artifacts).unwrap();
        let value = wallet
            .bitcoin_inputs_view(request.operation_id.clone())
            .unwrap();
        assert_eq!(value["txid"], txid);
        assert_eq!(value["operation_id"], request.operation_id);
        assert_eq!(value["view"]["total_sats"], "100000");
        assert_eq!(value["view"]["inputs"].as_array().unwrap().len(), 1);
        assert_eq!(wallet.bitcoin_spend_view().unwrap().total_sats, "20000");
        assert_eq!(
            serde_json::to_value(&wallet.transfer_artifacts).unwrap(),
            before
        );
        for id in ["invalid", "01900000-0000-7000-8000-000000000002"] {
            assert!(wallet.bitcoin_inputs_view(id.into()).is_err());
        }
    }

    #[test]
    fn exact_original_input_view_never_exempts_another_native_original() {
        let (mut wallet, request, txid) = fixture();
        let mut other = wallet.transfer_artifacts[&txid].clone();
        let record = other.bitcoin_recovery.as_mut().unwrap();
        let purpose = record.request.input_preparation.as_mut().unwrap();
        purpose.request.operation_id = "01900000-0000-7000-8000-000000000002".into();
        purpose.request.output_sats[0] = "4001".into();
        let target = ScriptBuf::from_hex(&purpose.output_scripts[0]).unwrap();
        let change = ScriptBuf::from_hex(&purpose.change_script).unwrap();
        let mut psbt = Psbt::from_str(&record.request.psbt).unwrap();
        for output in &mut psbt.unsigned_tx.output {
            if output.script_pubkey == target {
                output.value += BdkAmount::from_sat(1);
            }
            if output.script_pubkey == change {
                output.value -= BdkAmount::from_sat(1);
            }
        }
        record.request.psbt = psbt.to_string();
        let other_id = psbt.unsigned_tx.compute_txid().to_string();
        validate_bitcoin_recovery_artifact(&other_id, &other).unwrap();
        wallet.transfer_artifacts.insert(other_id, other);
        let before = serde_json::to_value(&wallet.transfer_artifacts).unwrap();
        let value = wallet.bitcoin_inputs_view(request.operation_id).unwrap();
        assert_eq!(value["view"]["inputs"], serde_json::json!([]));
        assert_eq!(value["view"]["total_sats"], "0");
        assert_eq!(
            serde_json::to_value(&wallet.transfer_artifacts).unwrap(),
            before
        );
        assert_eq!(wallet.bitcoin_spend_view().unwrap().total_sats, "0");
    }

    #[test]
    fn preparation_retains_owned_unsigned_original_and_holds_before_any_signature() {
        let (wallet, request, txid) = fixture();
        let data = wallet.bitcoin_inputs_data(request.operation_id).unwrap();
        assert_eq!(data["kind"], "wallet-bitcoin-input-preparation");
        assert_eq!(data["attempted"], false);
        assert_eq!(data["cancelled"], false);
        assert_eq!(data["own_signatures_complete"], false);
        assert!(data["signed_psbt"].is_null());
        assert_eq!(wallet.bitcoin_spend_view().unwrap().total_sats, "0");
        let bound = &wallet.transfer_artifacts[&txid]
            .bitcoin_recovery
            .as_ref()
            .unwrap()
            .request;
        let psbt = parse_preparation(bound).unwrap().0;
        assert_eq!(psbt.unsigned_tx.output.len(), 4);
        wallet
            .check_bitcoin_preparation_outputs(bound, &psbt)
            .unwrap();
        assert!(
            wallet
                .checked_bitcoin_psbt_for_recovery(&bound.psbt, false, None)
                .is_err()
        );
        assert!(
            wallet
                .checked_bitcoin_psbt_for_recovery(&bound.psbt, false, Some(&txid))
                .is_ok()
        );
    }

    #[test]
    fn exact_unsigned_cancel_does_not_erase_original_and_restores_only_its_held_inputs() {
        let (mut wallet, request, txid) = fixture();
        let bound = wallet.transfer_artifacts[&txid]
            .bitcoin_recovery
            .as_ref()
            .unwrap()
            .request
            .clone();
        wallet
            .cancel_bitcoin_inputs_before_flush(request.operation_id.clone(), bound.psbt.clone())
            .unwrap();
        assert_eq!(wallet.bitcoin_spend_view().unwrap().total_sats, "100000");
        assert!(wallet.transfer_artifacts.contains_key(&txid));
        assert_eq!(
            wallet
                .cancel_bitcoin_inputs_before_flush(
                    request.operation_id.clone(),
                    bound.psbt.clone()
                )
                .unwrap(),
            (txid, true),
            "an idempotent cancellation must preserve its already-cancelled flag on flush failure"
        );
        assert_eq!(
            wallet.bitcoin_inputs_data(request.operation_id).unwrap()["cancelled"],
            true
        );
        assert!(
            wallet
                .bitcoin_recovery_preflight(
                    &bound,
                    false,
                    super::super::super::safe_bitcoin::unix_seconds().unwrap()
                )
                .is_err()
        );
    }

    #[test]
    fn attempted_restored_or_changed_preparation_never_cancels() {
        for case in ["attempted", "restored", "changed"] {
            let (mut wallet, request, txid) = fixture();
            let record = wallet
                .transfer_artifacts
                .get_mut(&txid)
                .unwrap()
                .bitcoin_recovery
                .as_mut()
                .unwrap();
            let mut psbt = record.request.psbt.clone();
            match case {
                "attempted" => record.attempted = true,
                "restored" => record.restored = true,
                "changed" => psbt.push('x'),
                _ => unreachable!(),
            }
            let before = serde_json::to_value(&wallet.transfer_artifacts).unwrap();
            assert!(
                wallet
                    .cancel_bitcoin_inputs_before_flush(request.operation_id, psbt)
                    .is_err(),
                "{case}"
            );
            assert_eq!(
                serde_json::to_value(&wallet.transfer_artifacts).unwrap(),
                before
            );
            assert_eq!(wallet.bitcoin_spend_view().unwrap().total_sats, "0");
        }
    }

    #[test]
    fn standard_native_signatures_verify_and_replay_or_restored_dispatch_refuses() {
        let (mut wallet, request, txid) = fixture();
        let bound = wallet.transfer_artifacts[&txid]
            .bitcoin_recovery
            .as_ref()
            .unwrap()
            .request
            .clone();
        let now = super::super::super::safe_bitcoin::unix_seconds().unwrap();
        wallet
            .bitcoin_recovery_preflight(&bound, false, now)
            .unwrap();
        wallet
            .transfer_artifacts
            .get_mut(&txid)
            .unwrap()
            .bitcoin_recovery
            .as_mut()
            .unwrap()
            .attempted = true;
        let mut psbt = wallet
            .bitcoin_recovery_preflight(&bound, true, now)
            .unwrap();
        wallet._sign_psbt(&mut psbt, None).unwrap();
        wallet
            .transfer_artifacts
            .get_mut(&txid)
            .unwrap()
            .signed_psbt = Some(psbt.to_string());
        let record = wallet.transfer_artifacts[&txid]
            .bitcoin_recovery
            .as_ref()
            .unwrap();
        verify_original(record, &psbt.to_string()).unwrap();
        assert_eq!(
            wallet.bitcoin_inputs_data(request.operation_id).unwrap()["own_signatures_complete"],
            true
        );
        assert!(
            wallet
                .bitcoin_recovery_preflight(&bound, false, now)
                .is_err()
        );
        wallet
            .bitcoin_recovery_broadcast_preflight(&txid, false)
            .unwrap();
        wallet
            .transfer_artifacts
            .get_mut(&txid)
            .unwrap()
            .bitcoin_recovery
            .as_mut()
            .unwrap()
            .broadcast_attempted = true;
        assert!(
            wallet
                .bitcoin_recovery_broadcast_preflight(&txid, false)
                .is_err()
        );
        wallet
            .transfer_artifacts
            .get_mut(&txid)
            .unwrap()
            .bitcoin_recovery
            .as_mut()
            .unwrap()
            .restored = true;
        assert!(
            wallet
                .bitcoin_recovery_broadcast_preflight(&txid, true)
                .is_err()
        );
    }

    #[test]
    fn malformed_budgets_and_excluded_or_unconfirmed_funds_refuse() {
        let (_, original, _) = fixture();
        for case in [
            "one",
            "nine",
            "zero",
            "dust",
            "overflow",
            "fee",
            "network",
            "duplicate_hold",
        ] {
            let mut request = original.clone();
            match case {
                "one" => request.output_sats.truncate(1),
                "nine" => request.output_sats = vec!["4000".into(); 9],
                "zero" => request.output_sats[0] = "0".into(),
                "dust" => request.output_sats[0] = "329".into(),
                "overflow" => request.output_sats[0] = u64::MAX.to_string(),
                "fee" => request.maximum_fee_sats = "10001".into(),
                "network" => request.network = "mainnet".into(),
                "duplicate_hold" => {
                    request.excluded_inputs = vec![format!("{}:0", "a".repeat(64)); 2]
                }
                _ => unreachable!(),
            }
            assert!(validate_budget(&request).is_err(), "{case}");
        }
        for confirmed in [false, true] {
            let mut wallet = new_wallet();
            let point = coin(
                &mut wallet,
                KeychainKind::Internal,
                100_000,
                confirmed,
                false,
            );
            let mut request = original.clone();
            if confirmed {
                request.excluded_inputs.push(point.to_string());
            }
            assert!(wallet.build_bitcoin_inputs(request).is_err());
            assert!(wallet.transfer_artifacts.is_empty());
        }
        let mut wallet = new_wallet();
        coin(&mut wallet, KeychainKind::Internal, 100_000, true, false);
        let mut request = original;
        request.maximum_fee_sats = "1".into();
        assert!(wallet.build_bitcoin_inputs(request).is_err());
        assert!(wallet.transfer_artifacts.is_empty());
        assert_eq!(wallet.bitcoin_spend_view().unwrap().total_sats, "100000");
    }

    #[test]
    fn foreign_outputs_altered_values_and_legacy_purpose_do_not_authorize() {
        let (wallet, _, txid) = fixture();
        let original = wallet.transfer_artifacts[&txid]
            .bitcoin_recovery
            .as_ref()
            .unwrap()
            .request
            .clone();
        for case in ["value", "fee", "foreign", "legacy", "sighash", "metadata"] {
            let mut changed = original.clone();
            let mut psbt = parse_preparation(&changed).unwrap().0;
            match case {
                "value" => psbt.unsigned_tx.output[0].value += BdkAmount::from_sat(1),
                "fee" => changed.maximum_fee_sats = "1".into(),
                "foreign" => {
                    psbt.unsigned_tx.output[0].script_pubkey = new_wallet()
                        ._get_new_address(KeychainKind::Internal)
                        .unwrap()
                        .script_pubkey()
                }
                "legacy" => changed.input_preparation = None,
                "sighash" => {
                    psbt.inputs[0].sighash_type =
                        Some(bitcoin::sighash::TapSighashType::AllPlusAnyoneCanPay.into())
                }
                "metadata" => {
                    psbt.outputs[0].tap_internal_key = Some(
                        "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"
                            .parse()
                            .unwrap(),
                    )
                }
                _ => unreachable!(),
            }
            changed.psbt = psbt.to_string();
            assert!(parse_request(&changed).is_err(), "{case}");
        }
    }

    #[test]
    fn legacy_recovery_json_omits_new_purpose_and_cancellation_fields() {
        let (_, request, _) = super::super::tests::vector(1);
        let value = serde_json::to_value(&request).unwrap();
        assert!(value.get("input_preparation").is_none());
        let original = BitcoinRecoveryRecord {
            request,
            attempted: true,
            broadcast_attempted: false,
            restored: false,
            cancelled: false,
        };
        let encoded = serde_json::to_value(&original).unwrap();
        assert!(encoded.get("cancelled").is_none());
        let restored: BitcoinRecoveryRecord = serde_json::from_value(encoded).unwrap();
        assert_eq!(restored, original);
    }
}

/// Native-created exact output binding. Public deserialization is not a signing
/// capability: only the dedicated preparation APIs accept this purpose.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BitcoinInputPreparation {
    /// Original canonical budget, preserved before acknowledgement.
    pub request: BitcoinInputPreparationRequest,
    /// Native-created exact recipient scripts, in requested amount order.
    pub output_scripts: Vec<String>,
    /// Native-created ordinary Bitcoin change script, even if no change is due.
    pub change_script: String,
}

fn validate_budget(request: &BitcoinInputPreparationRequest) -> Result<(), Error> {
    if request.version != 1
        || request.network != "regtest"
        || !super::super::wallet_transfer::valid_operation_id(&request.operation_id)
        || !(2..=8).contains(&request.output_sats.len())
        || !(1..=1000).contains(&integer(&request.fee_rate)?)
        || !(1..=10000).contains(&integer(&request.maximum_fee_sats)?)
        || integer(&request.expires_at)? == 0
        || request.excluded_inputs.len() > 4096
    {
        return Err(refused());
    }
    let mut total = 0u64;
    for amount in &request.output_sats {
        let amount = integer(amount)?;
        if amount < 330 {
            return Err(refused());
        }
        total = total.checked_add(amount).ok_or_else(refused)?;
    }
    if total > BdkAmount::MAX_MONEY.to_sat() {
        return Err(refused());
    }
    let mut points = HashSet::new();
    for text in &request.excluded_inputs {
        if text.len() > 76 {
            return Err(refused());
        }
        let point = OutPoint::from_str(text).map_err(|_| refused())?;
        if point.to_string() != *text || !points.insert(point) {
            return Err(refused());
        }
    }
    Ok(())
}

pub(super) fn parse_preparation(
    request: &BitcoinRecoveryRequest,
) -> Result<(Psbt, HashSet<OutPoint>, u64), Error> {
    let purpose = request.input_preparation.as_ref().ok_or_else(refused)?;
    validate_budget(&purpose.request)?;
    if request.version != 1
        || request.network != "regtest"
        || request.fee_rate != purpose.request.fee_rate
        || request.maximum_fee_sats != purpose.request.maximum_fee_sats
        || request.expires_at != purpose.request.expires_at
        || request.psbt.len() > 180_000
        || purpose.output_scripts.len() != purpose.request.output_sats.len()
    {
        return Err(refused());
    }
    let psbt = Psbt::from_str(&request.psbt).map_err(|_| refused())?;
    if psbt.to_string() != request.psbt
        || psbt.version != 0
        || !psbt.xpub.is_empty()
        || !psbt.proprietary.is_empty()
        || !psbt.unknown.is_empty()
        || psbt.unsigned_tx.version != bitcoin::transaction::Version::TWO
        || !(1..=32).contains(&psbt.inputs.len())
        || psbt.inputs.len() != psbt.unsigned_tx.input.len()
        || psbt.unsigned_tx.output.len() < purpose.output_scripts.len()
        || psbt.unsigned_tx.output.len() > purpose.output_scripts.len() + 1
        || psbt
            .outputs
            .iter()
            .any(|output| output != &bitcoin::psbt::Output::default())
    {
        return Err(refused());
    }
    let script = |text: &str| -> Result<ScriptBuf, Error> {
        if text.len() != 68 {
            return Err(refused());
        }
        let value = ScriptBuf::from_hex(text).map_err(|_| refused())?;
        if !value.is_p2tr() || value.to_hex_string() != text {
            return Err(refused());
        }
        Ok(value)
    };
    let change = script(&purpose.change_script)?;
    let mut recipients = HashMap::new();
    for (text, amount) in purpose
        .output_scripts
        .iter()
        .zip(&purpose.request.output_sats)
    {
        let key = script(text)?;
        if key == change || recipients.insert(key, integer(amount)?).is_some() {
            return Err(refused());
        }
    }
    let mut seen_outputs = HashSet::new();
    let mut output_sats = 0u64;
    for output in &psbt.unsigned_tx.output {
        if !seen_outputs.insert(output.script_pubkey.clone())
            || !output.script_pubkey.is_p2tr()
            || output.value < TxOut::minimal_non_dust(output.script_pubkey.clone()).value
            || (output.script_pubkey != change
                && recipients.get(&output.script_pubkey) != Some(&output.value.to_sat()))
        {
            return Err(refused());
        }
        output_sats = output_sats
            .checked_add(output.value.to_sat())
            .ok_or_else(refused)?;
    }
    if recipients
        .keys()
        .any(|script| !seen_outputs.contains(script))
    {
        return Err(refused());
    }
    let mut points = HashSet::new();
    let mut input_sats = 0u64;
    for (txin, input) in psbt.unsigned_tx.input.iter().zip(&psbt.inputs) {
        let witness = input.witness_utxo.as_ref().ok_or_else(refused)?;
        let expected = bitcoin::psbt::Input {
            witness_utxo: Some(witness.clone()),
            ..Default::default()
        };
        if !points.insert(txin.previous_output)
            || input != &expected
            || purpose
                .request
                .excluded_inputs
                .contains(&txin.previous_output.to_string())
            || !txin.script_sig.is_empty()
            || !txin.witness.is_empty()
            || !witness.script_pubkey.is_p2tr()
            || witness.value.to_sat() == 0
            || input.final_script_sig.is_some()
            || input.final_script_witness.is_some()
            || input.tap_key_sig.is_some()
            || !input.partial_sigs.is_empty()
            || !input.tap_script_sigs.is_empty()
        {
            return Err(refused());
        }
        input_sats = input_sats
            .checked_add(witness.value.to_sat())
            .ok_or_else(refused)?;
    }
    let fee = input_sats.checked_sub(output_sats).ok_or_else(refused)?;
    if input_sats > BdkAmount::MAX_MONEY.to_sat()
        || fee == 0
        || fee > integer(&request.maximum_fee_sats)?
    {
        return Err(refused());
    }
    Ok((psbt, points, integer(&request.expires_at)?))
}

impl Wallet {
    fn bitcoin_preparation_by_operation(&self, id: &str) -> Result<Option<String>, Error> {
        if !super::super::wallet_transfer::valid_operation_id(id)
            || self.transfer_artifacts.len() > 32768
        {
            return Err(refused());
        }
        let mut found = None;
        for (txid, artifact) in &self.transfer_artifacts {
            let Some(record) = &artifact.bitcoin_recovery else {
                continue;
            };
            let Some(purpose) = &record.request.input_preparation else {
                continue;
            };
            validate_bitcoin_recovery_artifact(txid, artifact)?;
            if purpose.request.operation_id == id && found.replace(txid.clone()).is_some() {
                return Err(refused());
            }
        }
        Ok(found)
    }

    pub(super) fn check_bitcoin_preparation_outputs(
        &self,
        request: &BitcoinRecoveryRequest,
        psbt: &Psbt,
    ) -> Result<(), Error> {
        let original = parse_preparation(request)?.0;
        if original.unsigned_tx != psbt.unsigned_tx {
            return Err(refused());
        }
        for output in &psbt.unsigned_tx.output {
            if !matches!(
                self.bdk_wallet
                    .derivation_of_spk(output.script_pubkey.clone()),
                Some((KeychainKind::Internal, _))
            ) {
                return Err(refused());
            }
        }
        Ok(())
    }

    /// Build and retain one standard BDK self-payment before replying. No
    /// signature, broadcast or input-release acknowledgement is manufactured.
    pub async fn prepare_bitcoin_inputs(
        &mut self,
        online: Online,
        mut request: BitcoinInputPreparationRequest,
    ) -> Result<serde_json::Value, Error> {
        validate_budget(&request)?;
        request.excluded_inputs.sort();
        if let Some(txid) = self.bitcoin_preparation_by_operation(&request.operation_id)? {
            let old = self.transfer_artifacts[&txid]
                .bitcoin_recovery
                .as_ref()
                .unwrap();
            if old.request.input_preparation.as_ref().unwrap().request != request {
                return Err(refused());
            }
            self.flush().await?;
            return self.bitcoin_recovery_data(txid);
        }
        self.require_wallet_rgb_regtest(&online).await?;
        self.sync_db_txos(false).await?;
        let result = self.build_bitcoin_inputs(request)?;
        self.flush().await?;
        Ok(result)
    }

    pub(in crate::wallet) fn build_bitcoin_inputs(
        &mut self,
        request: BitcoinInputPreparationRequest,
    ) -> Result<serde_json::Value, Error> {
        validate_budget(&request)?;
        let now = super::super::safe_bitcoin::unix_seconds()?;
        let expiry = integer(&request.expires_at)?;
        if expiry <= now || expiry > now.checked_add(120).ok_or_else(refused)? {
            return Err(refused());
        }
        let view = self.bitcoin_spend_view()?;
        let allowed = view
            .inputs
            .iter()
            .map(|coin| OutPoint::from_str(&coin.outpoint).map_err(|_| refused()))
            .collect::<Result<HashSet<_>, _>>()?;
        let blocked = self
            .bdk_wallet
            .list_unspent()
            .filter(|coin| {
                !allowed.contains(&coin.outpoint)
                    || request.excluded_inputs.contains(&coin.outpoint.to_string())
            })
            .map(|coin| coin.outpoint)
            .collect::<Vec<_>>();
        let mut scripts = Vec::new();
        for _ in &request.output_sats {
            scripts.push(
                self._get_new_address(KeychainKind::Internal)?
                    .script_pubkey(),
            );
        }
        let change = self
            ._get_new_address(KeychainKind::Internal)?
            .script_pubkey();
        let rate = self._check_fee_rate(integer(&request.fee_rate)?)?;
        let mut builder = self.bdk_wallet.build_tx();
        builder
            .unspendable(blocked)
            .drain_to(change.clone())
            .fee_rate(rate);
        for (script, amount) in scripts.iter().zip(&request.output_sats) {
            builder.add_recipient(script.clone(), BdkAmount::from_sat(integer(amount)?));
        }
        let prepared = builder.finish().map_err(|_| refused())?;
        // Retain the same minimal key-spend PSBT used by Native self-recovery.
        // BDK finalization removes derivation metadata from its builder PSBT;
        // retaining that metadata would conflict with exact-original checking.
        // BDK still selects inputs and constructs the entire transaction. The
        // signer resolves our descriptors itself and the existing verifier
        // checks the resulting signatures against these exact witness UTXOs.
        let mut psbt = Psbt::from_unsigned_tx(prepared.unsigned_tx).map_err(|_| refused())?;
        for (input, selected) in psbt.inputs.iter_mut().zip(prepared.inputs) {
            input.witness_utxo = Some(selected.witness_utxo.ok_or_else(refused)?);
        }
        let txid = psbt.unsigned_tx.compute_txid().to_string();
        let bound = BitcoinRecoveryRequest {
            version: 1,
            network: "regtest".into(),
            psbt: psbt.to_string(),
            fee_rate: request.fee_rate.clone(),
            maximum_fee_sats: request.maximum_fee_sats.clone(),
            expires_at: request.expires_at.clone(),
            input_preparation: Some(BitcoinInputPreparation {
                request,
                output_scripts: scripts
                    .iter()
                    .map(|script| script.to_hex_string())
                    .collect(),
                change_script: change.to_hex_string(),
            }),
        };
        self.check_bitcoin_preparation_outputs(&bound, &psbt)?;
        self.checked_bitcoin_psbt_with_output_limit(&bound.psbt, false, None, 9)?;
        if self.transfer_artifacts.contains_key(&txid) {
            return Err(refused());
        }
        self.transfer_artifacts.insert(
            txid.clone(),
            TransferArtifacts {
                bitcoin_recovery: Some(BitcoinRecoveryRecord {
                    request: bound,
                    attempted: false,
                    broadcast_attempted: false,
                    restored: false,
                    cancelled: false,
                }),
                ..Default::default()
            },
        );
        self.bdk_wallet.persist(&mut self.bdk_database)?;
        self.bitcoin_recovery_data(txid)
    }

    /// Read the same original by wallet-local id, including after a missing ACK,
    /// expiry, lock/reload or complete-file restore. Never reconstructs a spend.
    pub fn bitcoin_inputs_data(&self, operation_id: String) -> Result<serde_json::Value, Error> {
        let txid = self
            .bitcoin_preparation_by_operation(&operation_id)?
            .ok_or_else(refused)?;
        self.bitcoin_recovery_data(txid)
    }

    /// Current qualification for only this retained original's inputs. The
    /// existing BDK/DB/Stock view exempts this one original, keeps all other
    /// holds and excludes spent, immature, unconfirmed or RGB-bearing coins.
    /// This observation never releases a hold or grants signing authority.
    pub fn bitcoin_inputs_view(&self, operation_id: String) -> Result<serde_json::Value, Error> {
        let txid = self
            .bitcoin_preparation_by_operation(&operation_id)?
            .ok_or_else(refused)?;
        let record = self.transfer_artifacts[&txid]
            .bitcoin_recovery
            .as_ref()
            .unwrap();
        let (_, points, _) = parse_request(&record.request)?;
        let mut view = self.bitcoin_preparation_input_view(&txid)?;
        view.inputs.retain(|coin| {
            points
                .iter()
                .any(|point| point.to_string() == coin.outpoint)
        });
        view.total_sats = view
            .inputs
            .iter()
            .try_fold(0u64, |sum, coin| {
                sum.checked_add(integer(&coin.amount_sats)?)
                    .ok_or_else(refused)
            })?
            .to_string();
        Ok(
            serde_json::json!({"version":1,"kind":"wallet-bitcoin-input-preparation-inputs",
            "operation_id":operation_id,"txid":txid,"view":view}),
        )
    }

    /// Sign once through the established durable Native self-payment barrier.
    pub async fn sign_bitcoin_inputs(
        &mut self,
        online: Online,
        operation_id: String,
        expected_psbt: String,
    ) -> Result<serde_json::Value, Error> {
        let txid = self
            .bitcoin_preparation_by_operation(&operation_id)?
            .ok_or_else(refused)?;
        let request = self.transfer_artifacts[&txid]
            .bitcoin_recovery
            .as_ref()
            .unwrap()
            .request
            .clone();
        if request.psbt != expected_psbt {
            return Err(refused());
        }
        self.sign_bitcoin_self_payment(online, request).await
    }

    /// Explicitly cancel only a proven unsigned, unrestored preparation. An
    /// attempted or uncertain signer/broadcast is never cancelled by time alone.
    pub async fn cancel_bitcoin_inputs(
        &mut self,
        operation_id: String,
        expected_psbt: String,
    ) -> Result<serde_json::Value, Error> {
        let (txid, was_cancelled) =
            self.cancel_bitcoin_inputs_before_flush(operation_id, expected_psbt)?;
        if let Err(error) = self.flush().await {
            self.transfer_artifacts
                .get_mut(&txid)
                .unwrap()
                .bitcoin_recovery
                .as_mut()
                .unwrap()
                .cancelled = was_cancelled;
            return Err(error);
        }
        self.bitcoin_recovery_data(txid)
    }

    fn cancel_bitcoin_inputs_before_flush(
        &mut self,
        operation_id: String,
        expected_psbt: String,
    ) -> Result<(String, bool), Error> {
        let txid = self
            .bitcoin_preparation_by_operation(&operation_id)?
            .ok_or_else(refused)?;
        let artifact = self.transfer_artifacts.get_mut(&txid).unwrap();
        let record = artifact.bitcoin_recovery.as_mut().unwrap();
        if record.request.psbt != expected_psbt
            || record.attempted
            || record.broadcast_attempted
            || record.restored
            || artifact.signed_psbt.is_some()
        {
            return Err(refused());
        }
        let was_cancelled = record.cancelled;
        record.cancelled = true;
        Ok((txid, was_cancelled))
    }

    /// One explicit dispatch of the same signed self-payment. The separate
    /// fresh broadcast approval never changes its original amount or recipients.
    pub async fn broadcast_bitcoin_inputs(
        &mut self,
        online: Online,
        operation_id: String,
        expected_txid: String,
        broadcast_expires_at: String,
    ) -> Result<String, Error> {
        let txid = self
            .bitcoin_preparation_by_operation(&operation_id)?
            .ok_or_else(refused)?;
        if txid != expected_txid {
            return Err(refused());
        }
        self.broadcast_bitcoin_self_payment(online, txid, broadcast_expires_at)
            .await
    }
}
