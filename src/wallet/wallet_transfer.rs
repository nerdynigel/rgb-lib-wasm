//! Wallet-owned ordinary RGB transfers reuse the external-send lifecycle. No
//! Stock consumption, signature, consignment delivery or broadcast in prepare.
use super::*;
use serde::{Deserializer, Serializer};

/// Explicit, bounded parameters for one ordinary REGTEST NIA transfer.
/// Amounts serialize as integer strings at the SDK boundary.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WalletRgbSendRequest {
    /// Stable wallet-local idempotency key (UUIDv4/v7).
    pub operation_id: String,
    /// Exact standard RGB ContractId, never ticker text.
    pub asset_id: String,
    /// Network-bound witness beneficiary from the recipient's RGB invoice.
    pub recipient_id: String,
    /// Integer RGB base units.
    #[serde(with = "integer_string")]
    pub amount: u64,
    /// Explicit Bitcoin carrier delivered to the witness beneficiary, in sats.
    #[serde(with = "integer_string")]
    pub witness_sats: u64,
    /// Recipient-required blinding, if any.
    #[serde(default, with = "optional_integer_string")]
    pub blinding: Option<u64>,
    /// Exact recipient transport endpoints.
    pub transport_endpoints: Vec<String>,
    /// Explicit whole sat/vB fee rate.
    #[serde(with = "integer_string")]
    pub fee_rate: u64,
    /// Required confirmation depth (nonzero).
    pub min_confirmations: u8,
    /// Other durable wallet input holds; excluded before selection.
    pub excluded_inputs: Vec<String>,
    /// Wallet-approved Unix-second preparation/signing expiry.
    #[serde(with = "integer_string")]
    pub expires_at: u64,
}

mod integer_string {
    use super::*;
    pub fn serialize<S: Serializer>(v: &u64, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&v.to_string())
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<u64, D::Error> {
        crate::utils::from_str_or_number_mandatory(d)
    }
}

mod optional_integer_string {
    use super::*;
    pub fn serialize<S: Serializer>(v: &Option<u64>, s: S) -> Result<S::Ok, S::Error> {
        match v {
            Some(value) => s.serialize_some(&value.to_string()),
            None => s.serialize_none(),
        }
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<u64>, D::Error> {
        crate::utils::from_str_or_number_optional(d)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub(crate) struct WalletTransferBinding {
    pub(crate) operation_id: String,
    pub(crate) request_json: String,
    pub(crate) signing_attempted: bool,
    pub(crate) broadcast_attempted: bool,
}

struct ValidatedRequest {
    binding: WalletTransferBinding,
    recipient: Recipient,
    script: ScriptBuf,
    contract: ContractId,
    excluded: HashSet<OutPoint>,
}

fn invalid() -> Error {
    Error::InvalidRecipientMap
}

fn valid_operation_id(id: &str) -> bool {
    let b = id.as_bytes();
    b.len() == 36
        && b.iter().enumerate().all(|(i, c)| {
            if [8, 13, 18, 23].contains(&i) {
                *c == b'-'
            } else {
                c.is_ascii_digit() || (b'a'..=b'f').contains(c)
            }
        })
        && b"47".contains(&b[14])
        && b"89ab".contains(&b[19])
}

fn validate_request(r: &mut WalletRgbSendRequest) -> Result<ValidatedRequest, Error> {
    if !valid_operation_id(&r.operation_id)
        || r.asset_id.len() > 192
        || r.recipient_id.len() > 512
        || r.amount == 0
        || !(330..=10_000).contains(&r.witness_sats)
        || !(1..=1000).contains(&r.fee_rate)
        || !(1..=144).contains(&r.min_confirmations)
        || r.expires_at == 0
        || r.expires_at > 9_007_199_254_740_991
        || r.excluded_inputs.len() > 256
        || r.transport_endpoints.is_empty()
        || r.transport_endpoints.len() > 4
        || r.transport_endpoints.iter().any(|s| s.len() > 2048)
    {
        return Err(invalid());
    }
    let contract = ContractId::from_str(&r.asset_id).map_err(|_| invalid())?;
    let beneficiary = XChainNet::<Beneficiary>::from_str(&r.recipient_id)
        .map_err(|_| Error::InvalidRecipientID)?;
    if beneficiary.chain_network() != ChainNet::from(BitcoinNetwork::Regtest) {
        return Err(Error::InvalidRecipientNetwork);
    }
    let Beneficiary::WitnessVout(pay_to, None) = beneficiary.into_inner() else {
        return Err(Error::InvalidRecipientData {
            details: s!(
                "ordinary wallet transfer requires a supported witness RGB receive request"
            ),
        });
    };
    let mut transports = HashSet::new();
    for endpoint in &r.transport_endpoints {
        if !transports.insert(endpoint.clone())
            || TransportEndpoint::new(endpoint.clone())?.transport_type != TransportType::JsonRpc
        {
            return Err(invalid());
        }
    }
    let mut excluded = HashSet::new();
    for point in &r.excluded_inputs {
        if point.len() > 76 {
            return Err(invalid());
        }
        let parsed = OutPoint::from_str(point).map_err(|_| invalid())?;
        if parsed.to_string() != *point || !excluded.insert(parsed) {
            return Err(invalid());
        }
    }
    r.excluded_inputs.sort();
    let request_json = serde_json::to_string(r).map_err(InternalError::from)?;
    if request_json.len() > 65_536 {
        return Err(invalid());
    }
    Ok(ValidatedRequest {
        binding: WalletTransferBinding {
            operation_id: r.operation_id.clone(),
            request_json,
            signing_attempted: false,
            broadcast_attempted: false,
        },
        recipient: Recipient {
            recipient_id: r.recipient_id.clone(),
            witness_data: Some(WitnessData {
                amount_sat: r.witness_sats,
                blinding: r.blinding,
            }),
            assignment: Assignment::Fungible(r.amount),
            transport_endpoints: r.transport_endpoints.clone(),
        },
        script: pay_to.to_script(),
        contract,
        excluded,
    })
}

impl Wallet {
    fn wallet_transfer_by_operation(
        &self,
        id: &str,
    ) -> Result<Option<(&String, &TransferArtifacts)>, Error> {
        if !valid_operation_id(id) || self.transfer_artifacts.len() > 4096 {
            return Err(invalid());
        }
        let mut matches = self.transfer_artifacts.iter().filter(|(_, a)| {
            a.external_send
                .as_ref()
                .and_then(|e| e.wallet_transfer.as_ref())
                .is_some_and(|b| b.operation_id == id)
        });
        let found = matches.next();
        if matches.next().is_some() {
            return Err(Error::ExternalSendConflict { txid: id.into() });
        }
        Ok(found)
    }
    /// Prepare an ordinary, wallet-owned NIA witness transfer using BDK selection,
    /// standard RGB colouring and the existing durable external-send lifecycle.
    /// The actual Stock is unchanged. A retry can only recover the exact original
    /// request; it cannot choose new inputs or regenerate a transaction.
    pub async fn prepare_wallet_rgb_send(
        &mut self,
        online: Online,
        mut request: WalletRgbSendRequest,
    ) -> Result<ExternalSendPrepareResult, Error> {
        if self.bitcoin_network() != BitcoinNetwork::Regtest {
            return Err(Error::InvalidRecipientNetwork);
        }
        self.check_online(online.clone())?;
        let valid = validate_request(&mut request)?;
        if let Some((txid, _)) = self.wallet_transfer_by_operation(&request.operation_id)? {
            let txid = txid.clone();
            let art = self.transfer_artifacts[&txid]
                .external_send
                .as_ref()
                .unwrap();
            let binding = art.wallet_transfer.as_ref().unwrap();
            if binding.request_json != valid.binding.request_json
                || matches!(art.status, ExternalSendStatus::Conflicted)
            {
                return Err(Error::ExternalSendConflict { txid });
            }
            let batch = self
                .batch_transfer_by_txid(&txid)?
                .ok_or_else(|| Error::ExternalSendUnknown { txid: txid.clone() })?;
            if batch.status.failed() {
                return Err(Error::ExternalSendConflict { txid });
            }
            self.flush().await?;
            return Ok(ExternalSendPrepareResult {
                txid,
                batch_transfer_idx: batch.idx,
            });
        }
        let n = u64::try_from(now().unix_timestamp()).map_err(|_| invalid())?;
        if request.expires_at <= n
            || request.expires_at > n.checked_add(86_400).ok_or_else(invalid)?
        {
            return Err(invalid());
        }
        if self
            .database
            .check_asset_exists(request.asset_id.clone())?
            .schema
            != AssetSchema::Nia
        {
            return Err(Error::InvalidAssignment);
        }
        let genesis = bitcoin::blockdata::constants::genesis_block(BdkNetwork::Regtest)
            .block_hash()
            .to_string();
        if self.indexer().block_hash(0).await? != genesis {
            return Err(Error::InvalidRecipientNetwork);
        }
        self.sync(online.clone()).await?;
        let (rate, _, mut unspents, runtime) =
            self._get_transfer_begin_data(online, request.fee_rate)?;
        drop(runtime);
        if unspents.len() > 4096 {
            return Err(invalid());
        }
        unspents.retain(|u| {
            !valid
                .excluded
                .iter()
                .any(|p| p.to_string() == u.utxo.outpoint().to_string())
                && u.utxo.exists
                && !u.utxo.spent
                && !u.utxo.pending_witness
                && u.pending_blinded == 0
                && !u.rgb_allocations.is_empty()
                && u.rgb_allocations.iter().all(|a| {
                    a.asset_id.as_deref() == Some(&request.asset_id)
                        && a.status.settled()
                        && matches!(a.assignment, Assignment::Fungible(_))
                })
        });
        // Check complete candidate totals before the legacy selector's additions.
        let mut total_rgb = 0u64;
        let mut total_btc = 0u64;
        for u in &unspents {
            total_btc = total_btc
                .checked_add(u.utxo.btc_amount.parse::<u64>().map_err(|_| invalid())?)
                .ok_or_else(invalid)?;
            for a in &u.rgb_allocations {
                let Assignment::Fungible(amount) = a.assignment else {
                    return Err(invalid());
                };
                total_rgb = total_rgb.checked_add(amount).ok_or_else(invalid)?;
            }
        }
        unspents.sort_by_key(|u| u.utxo.outpoint().to_string());
        let spend = self._select_rgb_inputs(
            request.asset_id.clone(),
            &AssignmentsCollection {
                fungible: request.amount,
                ..Default::default()
            },
            unspents,
        )?;
        let mut inputs = spend
            .txo_map
            .values()
            .map(|p| OutPoint::from_str(&p.to_string()).map_err(|_| invalid()))
            .collect::<Result<HashSet<_>, _>>()?;
        if inputs.is_empty() || inputs.len() > 128 {
            return Err(invalid());
        }
        self._restore_missing_bdk_inputs(&inputs).await?;
        let db_txos = self.database.iter_txos()?;
        if db_txos.len() > 32_768 {
            return Err(invalid());
        }
        let db_outpoints = db_txos
            .iter()
            .map(|u| OutPoint::from_str(&u.outpoint().to_string()).map_err(|_| invalid()))
            .collect::<Result<HashSet<_>, _>>()?;
        let mut fee_inputs: Vec<_> = self
            .internal_unspents()
            .filter(|u| {
                !valid.excluded.contains(&u.outpoint) && !db_outpoints.contains(&u.outpoint)
            })
            .take(4097)
            .collect();
        if fee_inputs.len() > 4096 {
            return Err(invalid());
        }
        fee_inputs.sort_by_key(|u| (u.txout.value.to_sat(), u.outpoint));
        let mut fees = fee_inputs.into_iter();
        let outputs = vec![(valid.script, request.witness_sats)];
        let (mut psbt, change) = loop {
            match self._prepare_psbt(inputs.clone(), &outputs, rate, None) {
                Ok((psbt, change))
                    if spend.assignments_collected.fungible == request.amount
                        || change.is_some() =>
                {
                    break (psbt, change);
                }
                Ok(_) | Err(Error::InsufficientBitcoins { .. }) => {
                    let next = fees.next().ok_or(Error::InsufficientBitcoins {
                        needed: request.witness_sats,
                        available: total_btc,
                    })?;
                    // Never silently spend an unconfirmed vanilla input.
                    if self
                        .indexer()
                        .get_tx_confirmations(&next.outpoint.txid.to_string())
                        .await?
                        .is_none_or(|depth| depth < u64::from(request.min_confirmations))
                    {
                        continue;
                    }
                    inputs.insert(next.outpoint);
                    if inputs.len() > 128 {
                        return Err(invalid());
                    }
                }
                Err(e) => return Err(e),
            }
        };
        if psbt
            .unsigned_tx
            .output
            .first()
            .is_none_or(|o| !o.script_pubkey.is_op_return())
        {
            return Err(invalid());
        }
        if psbt.unsigned_tx.output.get(1).is_none_or(|o| {
            o.script_pubkey != outputs[0].0 || o.value.to_sat() != request.witness_sats
        }) {
            return Err(invalid());
        }
        for input in &psbt.unsigned_tx.input {
            if valid.excluded.contains(&input.previous_output)
                || self
                    .indexer()
                    .get_tx_confirmations(&input.previous_output.txid.to_string())
                    .await?
                    .is_none_or(|depth| depth < u64::from(request.min_confirmations))
            {
                return Err(Error::InvalidPsbt {
                    details: s!("wallet input is reserved or insufficiently confirmed"),
                });
            }
        }
        let shift = u32::from(
            psbt.unsigned_tx
                .output
                .iter()
                .any(|o| o.script_pubkey.is_p2tr()),
        );
        let mut output_map =
            HashMap::from([(1u32.checked_sub(shift).ok_or_else(invalid)?, request.amount)]);
        let remainder = spend
            .assignments_collected
            .fungible
            .checked_sub(request.amount)
            .ok_or_else(invalid)?;
        if remainder > 0 {
            let change = change.ok_or_else(invalid)?;
            if !self.bdk_wallet.is_mine(
                psbt.unsigned_tx.output[change.vout as usize]
                    .script_pubkey
                    .clone(),
            ) {
                return Err(invalid());
            }
            output_map.insert(
                change.vout.checked_sub(shift).ok_or_else(invalid)?,
                remainder,
            );
        }
        let (fascia, _) = self.color_psbt(
            &mut psbt,
            super::rust_only::ColoringInfo {
                asset_info_map: HashMap::from([(
                    valid.contract,
                    super::rust_only::AssetColoringInfo {
                        output_map,
                        static_blinding: request.blinding,
                    },
                )]),
                static_blinding: None,
                nonce: None,
            },
        )?;
        self.prepare_external_rgb_send_bound(
            HashMap::from([(request.asset_id, vec![valid.recipient])]),
            &psbt,
            &fascia,
            request.min_confirmations,
            Some(valid.binding),
        )
        .await
    }

    /// Read the original prepared transfer by its wallet-local operation id.
    /// No new selection, signing, delivery, broadcast or Stock consumption.
    pub fn wallet_rgb_send_data(&self, operation_id: String) -> Result<serde_json::Value, Error> {
        if !valid_operation_id(&operation_id) {
            return Err(invalid());
        }
        let (txid, artifact) = self
            .wallet_transfer_by_operation(&operation_id)?
            .ok_or_else(|| Error::ExternalSendUnknown {
                txid: operation_id.clone(),
            })?;
        let binding = artifact
            .external_send
            .as_ref()
            .unwrap()
            .wallet_transfer
            .as_ref()
            .unwrap();
        let batch = self
            .batch_transfer_by_txid(txid)?
            .ok_or_else(|| Error::ExternalSendUnknown { txid: txid.clone() })?;
        let mut data = self.external_rgb_send_data(txid.clone())?;
        // Native transfer graphs contain u64 amounts encoded as JSON numbers.
        // Ordinary-send JS receives only opaque consignment/fascia bytes and
        // the exact integer-string request, never rounded parsed graph amounts.
        if let Some(object) = data.as_object_mut() {
            object.remove("transfers");
        }
        data["wallet_transfer"] = serde_json::to_value(binding).map_err(InternalError::from)?;
        data["batch_transfer_idx"] = serde_json::json!(batch.idx);
        data["batch_status"] = serde_json::to_value(batch.status).map_err(InternalError::from)?;
        Ok(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn empty_wallet() -> Wallet {
        let keys = generate_keys(BitcoinNetwork::Regtest);
        Wallet::new(WalletData {
            data_dir: "/tmp/rgb_wallet_transfer_unit".into(),
            bitcoin_network: BitcoinNetwork::Regtest,
            database_type: DatabaseType::Sqlite,
            max_allocations_per_utxo: 5,
            account_xpub_vanilla: keys.account_xpub_vanilla,
            account_xpub_colored: keys.account_xpub_colored,
            mnemonic: Some(keys.mnemonic),
            master_fingerprint: keys.master_fingerprint,
            vanilla_keychain: None,
            supported_schemas: vec![AssetSchema::Nia],
            reuse_addresses: false,
        })
        .unwrap()
    }
    fn artifact(psbt: &Psbt) -> TransferArtifacts {
        let mut req = request();
        TransferArtifacts {
            external_send: Some(ExternalSendArtifact {
                prepared_psbt: psbt.to_string(),
                expected_tx_hex: Wallet::tx_stripped_hex_from_unsigned(&psbt.unsigned_tx),
                fascia_json: "retained-original-fascia".into(),
                recipients_digest: "retained-original-recipients".into(),
                status: ExternalSendStatus::Prepared,
                conflict_txid: None,
                wallet_transfer: Some(validate_request(&mut req).unwrap().binding),
            }),
            ..Default::default()
        }
    }
    fn unsigned() -> Psbt {
        Psbt::from_unsigned_tx(bitcoin::Transaction {
            version: bitcoin::transaction::Version::TWO,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: vec![],
            output: vec![],
        })
        .unwrap()
    }
    fn request() -> WalletRgbSendRequest {
        let script = ScriptBuf::from_hex(
            "512079be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
        )
        .unwrap();
        WalletRgbSendRequest {
            operation_id: "01a19805-0462-759b-9050-b9b0feb9da8f".into(),
            asset_id: "rgb:Kpai9_2X-uhBiC5K-moZtGkh-hwThNSE-h96Pbck-gfJQRHY".into(),
            recipient_id: crate::utils::recipient_id_from_script_buf(
                script,
                BitcoinNetwork::Regtest,
            ),
            amount: 3,
            witness_sats: 1000,
            blinding: None,
            transport_endpoints: vec!["rpc://127.0.0.1:3000/json-rpc".into()],
            fee_rate: 3,
            min_confirmations: 1,
            excluded_inputs: vec![
                format!("{}:1", "a".repeat(64)),
                format!("{}:0", "b".repeat(64)),
            ],
            expires_at: 1_800_000_000,
        }
    }
    #[test]
    fn canonical_request_binds_money_recipient_contract_and_input_exclusions() {
        let mut original = request();
        let expected = validate_request(&mut original).unwrap();
        let mut reordered = original.clone();
        reordered.excluded_inputs.reverse();
        assert_eq!(
            validate_request(&mut reordered).unwrap().binding,
            expected.binding
        );
        let mut different = original.clone();
        different.amount += 1;
        assert_ne!(
            validate_request(&mut different).unwrap().binding,
            expected.binding
        );
        different = original.clone();
        different.witness_sats += 1;
        assert_ne!(
            validate_request(&mut different).unwrap().binding,
            expected.binding
        );
        different = original.clone();
        different.fee_rate += 1;
        assert_ne!(
            validate_request(&mut different).unwrap().binding,
            expected.binding
        );
        different = original.clone();
        different.excluded_inputs.pop();
        assert_ne!(
            validate_request(&mut different).unwrap().binding,
            expected.binding
        );
        assert_eq!(
            expected.recipient.assignment,
            Assignment::Fungible(original.amount)
        );
        assert_eq!(expected.excluded.len(), 2);
    }
    #[test]
    fn rejects_wrong_network_invalid_identity_duplicate_holds_and_transport_abuse() {
        let mut r = request();
        let script = validate_request(&mut r).unwrap().script;
        r.recipient_id =
            crate::utils::recipient_id_from_script_buf(script, BitcoinNetwork::Mainnet);
        assert!(matches!(
            validate_request(&mut r),
            Err(Error::InvalidRecipientNetwork)
        ));
        r = request();
        r.asset_id = "USDT".into();
        assert!(validate_request(&mut r).is_err());
        r = request();
        r.excluded_inputs.push(r.excluded_inputs[0].clone());
        assert!(validate_request(&mut r).is_err());
        r = request();
        r.excluded_inputs[0] = "../../secret".into();
        assert!(validate_request(&mut r).is_err());
        r = request();
        r.transport_endpoints.push(r.transport_endpoints[0].clone());
        assert!(validate_request(&mut r).is_err());
        r = request();
        r.transport_endpoints = vec!["x".repeat(2049)];
        assert!(validate_request(&mut r).is_err());
        r = request();
        r.transport_endpoints.clear();
        assert!(validate_request(&mut r).is_err());
    }
    #[test]
    fn rejects_zero_overflow_and_out_of_policy_bounds_before_any_wallet_mutation() {
        for (amount, carrier, fee, depth) in [
            (0, 1000, 3, 1),
            (3, 329, 3, 1),
            (3, 10001, 3, 1),
            (3, 1000, 0, 1),
            (3, 1000, 1001, 1),
            (3, 1000, 3, 0),
            (3, 1000, 3, 145),
        ] {
            let mut r = request();
            r.amount = amount;
            r.witness_sats = carrier;
            r.fee_rate = fee;
            r.min_confirmations = depth;
            assert!(validate_request(&mut r).is_err());
        }
        let mut encoded = serde_json::to_value(request()).unwrap();
        encoded["amount"] = serde_json::json!("18446744073709551616");
        assert!(serde_json::from_value::<WalletRgbSendRequest>(encoded).is_err());
        let mut encoded = serde_json::to_value(request()).unwrap();
        encoded["unapproved_recipient"] = serde_json::json!("someone else");
        assert!(serde_json::from_value::<WalletRgbSendRequest>(encoded).is_err());
    }
    #[test]
    fn wallet_operation_ids_are_bounded_canonical_uuids() {
        assert!(valid_operation_id("01a19805-0462-759b-9050-b9b0feb9da8f"));
        assert!(valid_operation_id("00000000-0000-4000-8000-000000000000"));
        for value in [
            "",
            "../wallet",
            "01A19805-0462-759b-9050-b9b0feb9da8f",
            "00000000-0000-1000-8000-000000000000",
            "00000000-0000-4000-0000-000000000000",
        ] {
            assert!(!valid_operation_id(value));
        }
    }
    #[test]
    fn monetary_wire_values_are_integer_strings_and_reject_floats() {
        #[derive(Deserialize, Serialize)]
        struct Units {
            #[serde(with = "integer_string")]
            value: u64,
        }
        let units = Units { value: u64::MAX };
        let encoded = serde_json::to_string(&units).unwrap();
        assert_eq!(encoded, "{\"value\":\"18446744073709551615\"}");
        assert_eq!(
            serde_json::from_str::<Units>(&encoded).unwrap().value,
            u64::MAX
        );
        for malformed in [
            "{\"value\":1.5}",
            "{\"value\":-1}",
            "{\"value\":\"18446744073709551616\"}",
        ] {
            assert!(serde_json::from_str::<Units>(malformed).is_err());
        }
    }
    #[test]
    fn wallet_binding_preserves_unknown_sign_and_broadcast_across_snapshot() {
        let original = WalletTransferBinding {
            operation_id: "00000000-0000-4000-8000-000000000000".into(),
            request_json: "exact-bound-request".into(),
            signing_attempted: true,
            broadcast_attempted: true,
        };
        let restored: WalletTransferBinding =
            serde_json::from_str(&serde_json::to_string(&original).unwrap()).unwrap();
        assert_eq!(restored, original);
    }
    #[test]
    fn generic_signing_refuses_managed_original_without_changing_its_records() {
        let mut wallet = empty_wallet();
        let psbt = unsigned();
        let txid = psbt.unsigned_tx.compute_txid().to_string();
        wallet
            .transfer_artifacts
            .insert(txid.clone(), artifact(&psbt));
        let batch = wallet
            .database
            .set_batch_transfer(DbBatchTransferActMod {
                txid: ActiveValue::Set(Some(txid.clone())),
                status: ActiveValue::Set(TransferStatus::Initiated),
                min_confirmations: ActiveValue::Set(1),
                created_at: ActiveValue::Set(1),
                expiration: ActiveValue::Set(Some(100)),
                ..Default::default()
            })
            .unwrap();
        let before = serde_json::to_value(&wallet.transfer_artifacts).unwrap();
        let result = wallet.sign_psbt(psbt.to_string(), None);
        assert!(matches!(result, Err(Error::InvalidPsbt { .. })));
        assert_eq!(
            serde_json::to_value(&wallet.transfer_artifacts).unwrap(),
            before
        );
        let data = wallet.wallet_rgb_send_data(request().operation_id).unwrap();
        assert_eq!(data["txid"], txid);
        assert_eq!(data["psbt"], psbt.to_string());
        assert_eq!(data["batch_transfer_idx"], batch);
        assert!(data.get("transfers").is_none());
        assert_eq!(data["wallet_transfer"]["signing_attempted"], false);
    }
    #[test]
    fn duplicate_operation_ids_in_corrupt_storage_fail_closed_instead_of_guessing() {
        let mut wallet = empty_wallet();
        let psbt = unsigned();
        wallet
            .transfer_artifacts
            .insert("first".into(), artifact(&psbt));
        wallet
            .transfer_artifacts
            .insert("second".into(), artifact(&psbt));
        let before = serde_json::to_value(&wallet.transfer_artifacts).unwrap();
        assert!(matches!(
            wallet.wallet_rgb_send_data(request().operation_id),
            Err(Error::ExternalSendConflict { .. })
        ));
        assert_eq!(
            serde_json::to_value(&wallet.transfer_artifacts).unwrap(),
            before
        );
    }
    #[test]
    fn historical_external_artifacts_without_wallet_binding_remain_readable() {
        let mut old = serde_json::to_value(artifact(&unsigned()).external_send.unwrap()).unwrap();
        old.as_object_mut().unwrap().remove("wallet_transfer");
        let original: ExternalSendArtifact = serde_json::from_value(old).unwrap();
        assert!(original.wallet_transfer.is_none());
        assert_eq!(original.status, ExternalSendStatus::Prepared);
        assert_eq!(original.fascia_json, "retained-original-fascia");
    }
    #[test]
    fn missing_persisted_batch_cannot_be_presented_as_a_recoverable_transfer() {
        let mut wallet = empty_wallet();
        let psbt = unsigned();
        wallet
            .transfer_artifacts
            .insert(psbt.unsigned_tx.compute_txid().to_string(), artifact(&psbt));
        let before = serde_json::to_value(&wallet.transfer_artifacts).unwrap();
        assert!(matches!(
            wallet.wallet_rgb_send_data(request().operation_id),
            Err(Error::ExternalSendUnknown { .. })
        ));
        assert_eq!(
            serde_json::to_value(&wallet.transfer_artifacts).unwrap(),
            before
        );
    }
}
