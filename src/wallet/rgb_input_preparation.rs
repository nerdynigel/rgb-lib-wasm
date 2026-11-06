//! Wallet-owned RGB output preparation uses the existing standard transfer,
//! original, signer, broadcaster and recovery gates. It is never a market fill.
use super::*;

/// One additional wallet-owned witness output in a bounded RGB preparation.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WalletRgbPreparationOutput {
    /// Exact REGTEST witness beneficiary already registered by this wallet.
    pub recipient_id: String,
    /// Positive integer RGB base units, encoded as a decimal string.
    #[serde(with = "integer_string")]
    pub amount: u64,
    /// Bitcoin carrier retained by this wallet, in integer sats.
    #[serde(with = "integer_string")]
    pub witness_sats: u64,
    /// Explicit witness blinding; must match the first output's policy.
    #[serde(default, with = "optional_integer_string")]
    pub blinding: Option<u64>,
    /// Existing standard RGB transport endpoints, exactly as approved.
    pub transport_endpoints: Vec<String>,
}

/// Additive purpose accepted only by the explicit wallet preparation API.
/// The base request describes the first output; all additional amounts count
/// towards selection, conservation and the immutable original request.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WalletRgbInputPreparation {
    /// Explicit purpose version, currently one.
    pub version: u8,
    /// One to seven more outputs, for two to eight independent allocations.
    pub additional_outputs: Vec<WalletRgbPreparationOutput>,
    /// Maximum miner fee for this self-transfer, never a market execution fee.
    #[serde(with = "integer_string")]
    pub maximum_fee_sats: u64,
}

pub(super) struct ValidatedOutput {
    pub(super) recipient: Recipient,
    pub(super) script: ScriptBuf,
    pub(super) amount: u64,
    pub(super) witness_sats: u64,
}

pub(super) fn requested_units(request: &WalletRgbSendRequest) -> Result<u64, Error> {
    let Some(preparation) = &request.input_preparation else {
        return Ok(request.amount);
    };
    let total = preparation
        .additional_outputs
        .iter()
        .try_fold(request.amount, |n, output| {
            n.checked_add(output.amount).ok_or_else(invalid)
        })?;
    if total == 0 || total > 100_000 {
        return Err(invalid());
    }
    Ok(total)
}

pub(super) fn validate_additional(
    request: &WalletRgbSendRequest,
) -> Result<Vec<ValidatedOutput>, Error> {
    let Some(preparation) = &request.input_preparation else {
        return Ok(vec![]);
    };
    if preparation.version != 1
        || request.recovery_of.is_some()
        || !(1..=7).contains(&preparation.additional_outputs.len())
        || !(1..=10_000).contains(&preparation.maximum_fee_sats)
    {
        return Err(invalid());
    }
    requested_units(request)?;
    let mut first = request.clone();
    first.input_preparation = None;
    let first = validate_request(&mut first)?;
    let mut scripts = HashSet::from([first.script]);
    let mut recipients = HashSet::from([request.recipient_id.clone()]);
    let mut result = Vec::with_capacity(preparation.additional_outputs.len());
    for output in &preparation.additional_outputs {
        if output.blinding != request.blinding
            || output.transport_endpoints != request.transport_endpoints
            || !recipients.insert(output.recipient_id.clone())
        {
            return Err(invalid());
        }
        // Reuse all standard recipient, network, integer, transport and input
        // exclusion validation. Clearing the purpose prevents recursion.
        let mut single = request.clone();
        single.input_preparation = None;
        single.recipient_id = output.recipient_id.clone();
        single.amount = output.amount;
        single.witness_sats = output.witness_sats;
        single.blinding = output.blinding;
        single.transport_endpoints = output.transport_endpoints.clone();
        let valid = validate_request(&mut single)?;
        if !scripts.insert(valid.script.clone()) {
            return Err(invalid());
        }
        result.push(ValidatedOutput {
            recipient: valid.recipient,
            script: valid.script,
            amount: output.amount,
            witness_sats: output.witness_sats,
        });
    }
    Ok(result)
}

impl Wallet {
    /// Prepare one standard RGB self-transfer into independently spendable
    /// wallet-owned outputs. No signing, posting, broadcast or Stock consumption.
    /// Repeated exact ids recover only the existing original through the common
    /// idempotency barrier; changed requests cannot replace it.
    pub async fn prepare_wallet_rgb_input_preparation(
        &mut self,
        online: Online,
        request: WalletRgbSendRequest,
    ) -> Result<ExternalSendPrepareResult, Error> {
        if request.input_preparation.is_none() || request.recovery_of.is_some() {
            return Err(invalid());
        }
        self.prepare_wallet_rgb_send_inner(online, request).await
    }

    pub(super) fn check_wallet_rgb_input_preparation(
        &self,
        request: &WalletRgbSendRequest,
        psbt: Option<&Psbt>,
    ) -> Result<(), Error> {
        let Some(preparation) = &request.input_preparation else {
            return Ok(());
        };
        if self.bitcoin_network() != BitcoinNetwork::Regtest {
            return Err(Error::InvalidRecipientNetwork);
        }
        let mut base = request.clone();
        base.input_preparation = None;
        let first = validate_request(&mut base)?;
        let mut outputs = vec![(first.script, request.amount, request.witness_sats)];
        outputs.extend(
            validate_additional(request)?
                .into_iter()
                .map(|output| (output.script, output.amount, output.witness_sats)),
        );
        let db = self.database.get_db_data(false)?;
        if db.transfers.len() > 32_768
            || db.asset_transfers.len() > 32_768
            || db.batch_transfers.len() > 32_768
        {
            return Err(invalid());
        }
        let pending = self.database.iter_pending_witness_scripts()?;
        let expected_txid = self
            .wallet_transfer_by_operation(&request.operation_id)?
            .map(|(txid, _)| txid.as_str());
        let beneficiaries = std::iter::once(request.recipient_id.as_str()).chain(
            preparation
                .additional_outputs
                .iter()
                .map(|output| output.recipient_id.as_str()),
        );
        for ((script, amount, _), recipient) in outputs.iter().zip(beneficiaries) {
            // Prove native descriptor ownership AND the actual own receive
            // record. A caller-supplied "self" flag or script is insufficient.
            if !matches!(
                self.bdk_wallet.derivation_of_spk(script.clone()),
                Some((KeychainKind::External, _))
            ) {
                return Err(invalid());
            }
            let incoming: Vec<_> = db
                .transfers
                .iter()
                .filter(|transfer| {
                    transfer.incoming && transfer.recipient_id.as_deref() == Some(recipient)
                })
                .collect();
            if incoming.len() != 1
                || incoming[0].requested_assignment != Some(Assignment::Fungible(*amount))
            {
                return Err(invalid());
            }
            let asset = db
                .asset_transfers
                .iter()
                .find(|asset| asset.idx == incoming[0].asset_transfer_idx)
                .ok_or_else(invalid)?;
            let batch = db
                .batch_transfers
                .iter()
                .find(|batch| batch.idx == asset.batch_transfer_idx)
                .ok_or_else(invalid)?;
            if asset.asset_id.as_deref() != Some(request.asset_id.as_str())
                || batch.status.failed()
                || batch.min_confirmations != request.min_confirmations
                || batch.expiration.is_none_or(|expiry| {
                    u64::try_from(expiry)
                        .ok()
                        .is_none_or(|expiry| expiry < request.expires_at)
                })
                || batch
                    .txid
                    .as_deref()
                    .is_some_and(|id| Some(id) != expected_txid)
                || (batch.txid.is_none()
                    && !pending
                        .iter()
                        .any(|row| row.script == script.to_hex_string()))
            {
                return Err(invalid());
            }
        }
        let Some(psbt) = psbt else { return Ok(()) };
        if psbt.unsigned_tx.output.len() < outputs.len() + 1
            || psbt.unsigned_tx.output.len() > outputs.len() + 3
            || psbt.inputs.is_empty()
            || psbt.inputs.len() > 128
            || psbt.inputs.len() != psbt.unsigned_tx.input.len()
        {
            return Err(invalid());
        }
        for (script, _, sats) in &outputs {
            if psbt
                .unsigned_tx
                .output
                .iter()
                .filter(|actual| actual.script_pubkey == *script && actual.value.to_sat() == *sats)
                .count()
                != 1
            {
                return Err(invalid());
            }
        }
        let mut commitments = 0;
        let output_sats = psbt
            .unsigned_tx
            .output
            .iter()
            .try_fold(0u64, |sum, output| {
                if output.script_pubkey.is_op_return() {
                    commitments += 1;
                    if output.value.to_sat() != 0 {
                        return Err(invalid());
                    }
                } else if self
                    .bdk_wallet
                    .derivation_of_spk(output.script_pubkey.clone())
                    .is_none()
                {
                    return Err(invalid());
                }
                sum.checked_add(output.value.to_sat()).ok_or_else(invalid)
            })?;
        let mut seen = HashSet::new();
        let input_sats = psbt.unsigned_tx.input.iter().zip(&psbt.inputs).try_fold(
            0u64,
            |sum, (txin, input)| {
                let native = self
                    .bdk_wallet
                    .get_utxo(txin.previous_output)
                    .ok_or_else(invalid)?;
                let ChainPosition::Confirmed { anchor, .. } = native.chain_position else {
                    return Err(invalid());
                };
                let depth = self
                    .bdk_wallet
                    .latest_checkpoint()
                    .height()
                    .checked_sub(anchor.block_id.height)
                    .and_then(|depth| depth.checked_add(1))
                    .ok_or_else(invalid)?;
                let previous = self
                    .bdk_wallet
                    .get_tx(txin.previous_output.txid)
                    .ok_or_else(invalid)?;
                if !seen.insert(txin.previous_output)
                    || depth < u32::from(request.min_confirmations)
                    || (previous.tx_node.tx.is_coinbase() && depth < 100)
                    || input.witness_utxo.as_ref() != Some(&native.txout)
                {
                    return Err(invalid());
                }
                sum.checked_add(native.txout.value.to_sat())
                    .ok_or_else(invalid)
            },
        )?;
        let fee = input_sats.checked_sub(output_sats).ok_or_else(invalid)?;
        if commitments != 1
            || input_sats > BdkAmount::MAX_MONEY.to_sat()
            || fee == 0
            || fee > preparation.maximum_fee_sats
        {
            return Err(invalid());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    //! Real native descriptors, BDK graph and database; invented local coins
    //! and receive records exercise authorization only, never RGB settlement.
    use super::*;
    use crate::wallet::online::safe_bitcoin::tests::{coin, wallet as native_wallet};

    fn receive(wallet: &mut Wallet, amount: u64) -> WalletRgbPreparationOutput {
        let script = wallet
            ._get_new_address(KeychainKind::External)
            .unwrap()
            .script_pubkey();
        let recipient_id =
            crate::utils::recipient_id_from_script_buf(script.clone(), BitcoinNetwork::Regtest);
        let batch = wallet
            .database
            .set_batch_transfer(DbBatchTransferActMod {
                status: ActiveValue::Set(TransferStatus::WaitingCounterparty),
                min_confirmations: ActiveValue::Set(1),
                expiration: ActiveValue::Set(Some(1_800_000_600)),
                ..Default::default()
            })
            .unwrap();
        let asset = wallet
            .database
            .set_asset_transfer(DbAssetTransferActMod {
                batch_transfer_idx: ActiveValue::Set(batch),
                asset_id: ActiveValue::Set(Some(
                    "rgb:Kpai9_2X-uhBiC5K-moZtGkh-hwThNSE-h96Pbck-gfJQRHY".into(),
                )),
                user_driven: ActiveValue::Set(true),
                ..Default::default()
            })
            .unwrap();
        wallet
            .database
            .set_transfer(DbTransferActMod {
                asset_transfer_idx: ActiveValue::Set(asset),
                incoming: ActiveValue::Set(true),
                requested_assignment: ActiveValue::Set(Some(Assignment::Fungible(amount))),
                recipient_id: ActiveValue::Set(Some(recipient_id.clone())),
                ..Default::default()
            })
            .unwrap();
        wallet
            .database
            .set_pending_witness_script(DbPendingWitnessScriptActMod {
                script: ActiveValue::Set(script.to_hex_string()),
                ..Default::default()
            })
            .unwrap();
        WalletRgbPreparationOutput {
            recipient_id,
            amount,
            witness_sats: 1000,
            blinding: None,
            transport_endpoints: vec!["rpc://127.0.0.1:3000/json-rpc".into()],
        }
    }

    fn fixture() -> (Wallet, WalletRgbSendRequest, Psbt) {
        fixture_with_confirmation(true)
    }

    fn fixture_with_confirmation(confirmed: bool) -> (Wallet, WalletRgbSendRequest, Psbt) {
        let mut wallet = native_wallet();
        let point = coin(
            &mut wallet,
            KeychainKind::Internal,
            101_000,
            confirmed,
            false,
        );
        let first = receive(&mut wallet, 1);
        let extra = vec![receive(&mut wallet, 1), receive(&mut wallet, 1)];
        let request = WalletRgbSendRequest {
            operation_id: "01a19805-0462-759b-9050-b9b0feb9da8f".into(),
            asset_id: "rgb:Kpai9_2X-uhBiC5K-moZtGkh-hwThNSE-h96Pbck-gfJQRHY".into(),
            recipient_id: first.recipient_id,
            amount: first.amount,
            witness_sats: first.witness_sats,
            blinding: None,
            transport_endpoints: first.transport_endpoints,
            fee_rate: 1,
            min_confirmations: 1,
            excluded_inputs: vec![],
            expires_at: 1_800_000_000,
            recovery_of: None,
            input_preparation: Some(WalletRgbInputPreparation {
                version: 1,
                additional_outputs: extra,
                maximum_fee_sats: 1000,
            }),
        };
        let mut copied = request.clone();
        let valid = validate_request(&mut copied).unwrap();
        let mut outputs = vec![TxOut {
            value: BdkAmount::ZERO,
            script_pubkey: ScriptBuf::from_bytes(vec![0x6a, 0x01, 0x00]),
        }];
        outputs.push(TxOut {
            value: BdkAmount::from_sat(1000),
            script_pubkey: valid.script,
        });
        outputs.extend(valid.additional.into_iter().map(|out| TxOut {
            value: BdkAmount::from_sat(out.witness_sats),
            script_pubkey: out.script,
        }));
        outputs.push(TxOut {
            value: BdkAmount::from_sat(97_000),
            script_pubkey: wallet
                ._get_new_address(KeychainKind::Internal)
                .unwrap()
                .script_pubkey(),
        });
        let mut psbt = Psbt::from_unsigned_tx(BdkTransaction {
            version: bitcoin::transaction::Version::TWO,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: vec![bitcoin::TxIn {
                previous_output: point,
                ..Default::default()
            }],
            output: outputs,
        })
        .unwrap();
        psbt.inputs[0].witness_utxo = Some(wallet.bdk_wallet.get_utxo(point).unwrap().txout);
        (wallet, request, psbt)
    }

    #[test]
    fn purpose_binds_all_outputs_and_fee_without_changing_old_request_bytes() {
        let (_, mut request, _) = fixture();
        let valid = validate_request(&mut request).unwrap();
        assert_eq!(requested_units(&request).unwrap(), 3);
        assert_eq!(valid.additional.len(), 2);
        let encoded = valid.binding.request_json;
        request.input_preparation.as_mut().unwrap().maximum_fee_sats = 999;
        assert_ne!(
            validate_request(&mut request).unwrap().binding.request_json,
            encoded
        );
        request.input_preparation = None;
        let old = validate_request(&mut request).unwrap().binding.request_json;
        assert!(!old.contains("input_preparation"));
        assert_eq!(
            serde_json::to_string(&serde_json::from_str::<WalletRgbSendRequest>(&old).unwrap())
                .unwrap(),
            old
        );
    }

    #[test]
    fn malformed_duplicate_overflow_and_unbounded_preparations_refuse() {
        let (_, original, _) = fixture();
        for case in [
            "empty",
            "too_many",
            "version",
            "no_fee",
            "high_fee",
            "zero",
            "overflow",
            "total",
            "duplicate",
            "carrier",
            "transport",
        ] {
            let mut request = original.clone();
            let preparation = request.input_preparation.as_mut().unwrap();
            match case {
                "empty" => preparation.additional_outputs.clear(),
                "too_many" => {
                    preparation.additional_outputs =
                        vec![preparation.additional_outputs[0].clone(); 8]
                }
                "version" => preparation.version = 2,
                "no_fee" => preparation.maximum_fee_sats = 0,
                "high_fee" => preparation.maximum_fee_sats = 10_001,
                "zero" => preparation.additional_outputs[0].amount = 0,
                "overflow" => preparation.additional_outputs[0].amount = u64::MAX,
                "total" => preparation.additional_outputs[0].amount = 100_000,
                "duplicate" => {
                    preparation.additional_outputs[0].recipient_id = request.recipient_id.clone()
                }
                "carrier" => preparation.additional_outputs[0].witness_sats = 329,
                "transport" => {
                    preparation.additional_outputs[0].transport_endpoints =
                        vec!["rpc://127.0.0.1:3001/json-rpc".into()]
                }
                _ => unreachable!(),
            }
            assert!(validate_request(&mut request).is_err(), "{case}");
        }
        let mut encoded = serde_json::to_value(&original).unwrap();
        encoded["input_preparation"]["unapproved"] = serde_json::json!(true);
        assert!(serde_json::from_value::<WalletRgbSendRequest>(encoded).is_err());
    }

    #[test]
    fn native_owns_every_receive_and_checks_real_fee_inputs() {
        let (wallet, request, psbt) = fixture();
        let before = serde_json::to_string(wallet.database.as_ref()).unwrap();
        wallet
            .check_wallet_rgb_input_preparation(&request, None)
            .unwrap();
        wallet
            .check_wallet_rgb_input_preparation(&request, Some(&psbt))
            .unwrap();
        assert_eq!(
            serde_json::to_string(wallet.database.as_ref()).unwrap(),
            before
        );
        let mut changed = request.clone();
        let mut foreign = native_wallet();
        changed.recipient_id = receive(&mut foreign, 1).recipient_id;
        assert!(
            wallet
                .check_wallet_rgb_input_preparation(&changed, None)
                .is_err()
        );
        let mut mainnet = request.clone();
        let valid = validate_request(&mut mainnet).unwrap();
        mainnet.recipient_id =
            crate::utils::recipient_id_from_script_buf(valid.script, BitcoinNetwork::Mainnet);
        assert!(validate_request(&mut mainnet).is_err());
    }

    #[test]
    fn own_receive_metadata_cannot_be_missing_changed_or_used_by_another_original() {
        for case in [
            "expiry",
            "missing_expiry",
            "failed",
            "other_original",
            "confirmations",
            "amount",
        ] {
            let (wallet, mut request, psbt) = fixture();
            let db = wallet.database.get_db_data(false).unwrap();
            if case == "confirmations" {
                // Confirmation depth is immutable in the DB update API. Vary
                // the request instead and prove the intended mismatch exists.
                request.min_confirmations = 2;
                assert_eq!(db.batch_transfers[0].min_confirmations, 1);
            } else if case == "amount" {
                let mut transfer: DbTransferActMod = db.transfers[0].clone().into();
                transfer.requested_assignment = ActiveValue::Set(Some(Assignment::Fungible(2)));
                wallet.database.update_transfer(&mut transfer).unwrap();
            } else {
                let mut batch: DbBatchTransferActMod = db.batch_transfers[0].clone().into();
                match case {
                    "expiry" => batch.expiration = ActiveValue::Set(Some(1_799_999_999)),
                    "missing_expiry" => batch.expiration = ActiveValue::Set(None),
                    "failed" => batch.status = ActiveValue::Set(TransferStatus::Failed),
                    "other_original" => batch.txid = ActiveValue::Set(Some("b".repeat(64))),
                    _ => unreachable!(),
                }
                wallet.database.update_batch_transfer(&mut batch).unwrap();
            }
            let before = serde_json::to_string(wallet.database.as_ref()).unwrap();
            assert!(
                wallet
                    .check_wallet_rgb_input_preparation(&request, Some(&psbt))
                    .is_err(),
                "{case}"
            );
            assert_eq!(
                serde_json::to_string(wallet.database.as_ref()).unwrap(),
                before
            );
        }
    }

    #[test]
    fn ordinary_send_cannot_accept_preparation_and_preparation_requires_explicit_purpose() {
        let (mut wallet, request, _) = fixture();
        let before = serde_json::to_string(wallet.database.as_ref()).unwrap();
        let online = Online {
            id: 0,
            indexer_url: "http://127.0.0.1:1".into(),
        };
        let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
        {
            let mut future =
                std::pin::pin!(wallet.prepare_wallet_rgb_send(online.clone(), request.clone()));
            assert!(matches!(
                std::future::Future::poll(future.as_mut(), &mut cx),
                std::task::Poll::Ready(Err(Error::InvalidRecipientMap))
            ));
        }
        let mut without_purpose = request;
        without_purpose.input_preparation = None;
        {
            let mut future = std::pin::pin!(
                wallet.prepare_wallet_rgb_input_preparation(online, without_purpose)
            );
            assert!(matches!(
                std::future::Future::poll(future.as_mut(), &mut cx),
                std::task::Poll::Ready(Err(Error::InvalidRecipientMap))
            ));
        }
        assert_eq!(
            serde_json::to_string(wallet.database.as_ref()).unwrap(),
            before
        );
    }

    #[test]
    fn changed_outputs_forged_values_and_excess_fees_never_authorize() {
        let (wallet, request, original) = fixture();
        for case in [
            "fee",
            "target",
            "witness",
            "missing_witness",
            "foreign_change",
            "duplicate_input",
            "commitment",
        ] {
            let mut changed = original.clone();
            match case {
                "fee" => {
                    changed.unsigned_tx.output.last_mut().unwrap().value -= BdkAmount::from_sat(1)
                }
                "target" => changed.unsigned_tx.output[1].value += BdkAmount::from_sat(1),
                "witness" => {
                    changed.inputs[0].witness_utxo.as_mut().unwrap().value += BdkAmount::from_sat(1)
                }
                "missing_witness" => changed.inputs[0].witness_utxo = None,
                "foreign_change" => {
                    changed.unsigned_tx.output.last_mut().unwrap().script_pubkey = native_wallet()
                        ._get_new_address(KeychainKind::Internal)
                        .unwrap()
                        .script_pubkey()
                }
                "duplicate_input" => {
                    changed
                        .unsigned_tx
                        .input
                        .push(changed.unsigned_tx.input[0].clone());
                    changed.inputs.push(changed.inputs[0].clone());
                }
                "commitment" => changed.unsigned_tx.output[0].value = BdkAmount::from_sat(1),
                _ => unreachable!(),
            }
            assert!(
                wallet
                    .check_wallet_rgb_input_preparation(&request, Some(&changed))
                    .is_err(),
                "{case}"
            );
        }
    }

    #[test]
    fn unconfirmed_native_inputs_never_authorize_preparation_signing() {
        let (wallet, request, psbt) = fixture_with_confirmation(false);
        assert!(
            wallet
                .check_wallet_rgb_input_preparation(&request, Some(&psbt))
                .is_err()
        );
    }
}
