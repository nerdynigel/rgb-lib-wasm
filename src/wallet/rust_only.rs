//! RGB Rust-only methods module
//!
//! This module defines additional utility methods that are not exposed via FFI

use super::*;

/// RGB asset-specific information to color a transaction
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AssetColoringInfo {
    /// Map of vouts and asset amounts to color the transaction outputs
    pub output_map: HashMap<u32, u64>,
    /// Static blinding to keep the transaction construction deterministic
    pub static_blinding: Option<u64>,
}

/// RGB information to color a transaction
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ColoringInfo {
    /// Asset-specific information
    pub asset_info_map: HashMap<ContractId, AssetColoringInfo>,
    /// Static blinding to keep the transaction construction deterministic
    pub static_blinding: Option<u64>,
    /// Nonce for offchain TXs ordering
    pub nonce: Option<u64>,
}

/// Map of contract ID and list of its beneficiaries
pub type AssetBeneficiariesMap = BTreeMap<ContractId, Vec<BuilderSeal<GraphSeal>>>;

/// Indexer protocol
#[derive(Clone, Debug)]
pub enum IndexerProtocol {
    /// An indexer implementing the esplora protocol
    Esplora,
}

impl fmt::Display for IndexerProtocol {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

/// Result of consignment validation (offchain or indexer-based).
#[cfg(feature = "esplora")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidateConsignmentResult {
    /// Whether the consignment is valid.
    pub valid: bool,
    /// Warnings from validation (when valid).
    pub warnings: Option<Vec<String>>,
    /// Error category (when invalid): "invalid" or "resolver".
    pub error: Option<String>,
    /// Detailed error/failure description (when invalid).
    pub details: Option<String>,
}

impl Wallet {
    /// Return all contract IDs currently held in the RGB stock.
    pub fn rgb_contract_ids(&self) -> Result<Vec<ContractId>, Error> {
        let runtime = self.rgb_runtime()?;
        Ok(runtime
            .contracts()?
            .into_iter()
            .map(|contract| contract.id)
            .collect())
    }

    /// Color a PSBT.
    ///
    /// <div class="warning">This method is meant for special usage and is normally not needed, use
    /// it only if you know what you're doing</div>
    pub fn color_psbt(
        &self,
        psbt: &mut Psbt,
        coloring_info: ColoringInfo,
    ) -> Result<(Fascia, AssetBeneficiariesMap), Error> {
        info!(self.logger, "Coloring PSBT...");
        let mut transaction = match psbt.clone().extract_tx() {
            Ok(tx) => tx,
            Err(ExtractTxError::MissingInputValue { tx }) => tx, // required for non-standard TXs
            Err(e) => return Err(InternalError::from(e).into()),
        };
        let mut opreturn_first = false;
        if transaction.output.iter().any(|o| o.script_pubkey.is_p2tr()) {
            opreturn_first = true;
        }

        if !transaction
            .output
            .iter()
            .any(|o| o.script_pubkey.is_op_return())
        {
            let opreturn_output = TxOut {
                value: BdkAmount::ZERO,
                script_pubkey: ScriptBuf::new_op_return([]),
            };
            if opreturn_first {
                transaction.output.insert(0, opreturn_output);
            } else {
                transaction.output.push(opreturn_output);
            }
            *psbt = Psbt::from_unsigned_tx(transaction).unwrap();
        }

        let runtime = self.rgb_runtime()?;

        let prev_outputs = psbt
            .unsigned_tx
            .input
            .iter()
            .map(|txin| txin.previous_output)
            .collect::<HashSet<OutPoint>>();

        let mut all_transitions: HashMap<ContractId, Transition> = HashMap::new();
        let mut asset_beneficiaries: AssetBeneficiariesMap = bmap![];
        let assignment_name = FieldName::from(RGB_STATE_ASSET_OWNER);

        for (contract_id, asset_coloring_info) in coloring_info.asset_info_map.clone() {
            let schema = AssetSchema::get_from_contract_id(contract_id, &runtime)?;

            let mut asset_transition_builder =
                runtime.transition_builder(contract_id, "transfer")?;

            let mut asset_available_amt = 0;
            for (_, opout_state_map) in
                runtime.contract_assignments_for(contract_id, prev_outputs.iter().copied())?
            {
                for (opout, state) in opout_state_map {
                    if let AllocatedState::Amount(amt) = &state {
                        asset_available_amt += amt.as_u64();
                    }
                    asset_transition_builder = asset_transition_builder.add_input(opout, state)?;
                }
            }

            let mut beneficiaries = vec![];
            let mut sending_amt = 0;
            for (mut vout, amount) in asset_coloring_info.output_map {
                if amount == 0 {
                    continue;
                }
                if opreturn_first {
                    vout += 1;
                }
                sending_amt += amount;
                if vout as usize >= psbt.outputs.len() {
                    return Err(Error::InvalidColoringInfo {
                        details: s!("invalid vout in output_map, does not exist in the given PSBT"),
                    });
                }
                let graph_seal = if let Some(blinding) = asset_coloring_info.static_blinding {
                    GraphSeal::with_blinded_vout(vout, blinding)
                } else {
                    GraphSeal::new_random_vout(vout)
                };
                let seal = BuilderSeal::Revealed(graph_seal);
                beneficiaries.push(seal);

                match schema {
                    AssetSchema::Nia | AssetSchema::Ifa => {
                        asset_transition_builder = asset_transition_builder.add_fungible_state(
                            assignment_name.clone(),
                            seal,
                            amount,
                        )?;
                    }
                }
            }
            if sending_amt > asset_available_amt {
                return Err(Error::InvalidColoringInfo {
                    details: format!(
                        "total amount in output_map ({sending_amt}) greater than available ({asset_available_amt})"
                    ),
                });
            }

            if let Some(nonce) = coloring_info.nonce {
                asset_transition_builder = asset_transition_builder.set_nonce(nonce);
            }

            let transition = asset_transition_builder.complete_transition()?;
            all_transitions.insert(contract_id, transition);
            asset_beneficiaries.insert(contract_id, beneficiaries);
        }

        let opreturn_index = psbt
            .unsigned_tx
            .output
            .iter()
            .enumerate()
            .find(|(_, o)| o.script_pubkey.is_op_return())
            .expect("psbt should have an op_return output")
            .0;
        let opreturn_output = psbt.outputs.get_mut(opreturn_index).unwrap();
        opreturn_output.set_opret_host();
        if let Some(blinding) = coloring_info.static_blinding {
            opreturn_output
                .set_mpc_entropy(blinding)
                .map_err(InternalError::from)?;
        }

        for (contract_id, transition) in all_transitions {
            for opout in transition.inputs() {
                psbt.set_rgb_contract_consumer(contract_id, opout, transition.id())
                    .map_err(InternalError::from)?;
            }
            psbt.push_rgb_transition(transition)
                .map_err(InternalError::from)?;
        }

        psbt.set_rgb_close_method(CloseMethod::OpretFirst);
        psbt.set_as_unmodifiable();
        let fascia = psbt.rgb_commit().map_err(|e| Error::Internal {
            details: e.to_string(),
        })?;

        info!(self.logger, "Color PSBT completed");
        Ok((fascia, asset_beneficiaries))
    }

    /// Color a prospective PSBT and export its transfers without consuming RGB state.
    ///
    /// Uses the wallet's existing stock and the engine's `transfer_from_fascia` derivation.
    /// All nonzero beneficiaries (including change and fees) are exported from the same
    /// committed fascia. Does not consume, flush, or register high-level transfers.
    /// Runtime acquisition/drop temporarily moves stock through the shared wallet slot;
    /// wallet construction and caller persistence remain separate operations.
    ///
    /// This is not approval or ownership-policy validation: callers must validate every
    /// allocation against their immutable reviewed transaction and resolve foreign prevouts.
    /// On error the caller's PSBT remains unchanged. No signing or broadcast is performed.
    pub fn color_psbt_and_export(
        &self,
        psbt: &mut Psbt,
        coloring_info: ColoringInfo,
    ) -> Result<Vec<RgbTransfer>, Error> {
        if coloring_info.asset_info_map.is_empty() {
            return Err(Error::InvalidColoringInfo {
                details: s!("no assets requested for transfer export"),
            });
        }
        // Refuse an active runtime instead of initializing a replacement empty stock.
        // This synchronous path cannot yield between this check and runtime acquisition.
        let stock = self.rgb_stock.try_borrow().map_err(|_| Error::Internal {
            details: s!("RGB runtime already in use"),
        })?;
        if stock.is_none() {
            return Err(Error::Internal {
                details: s!(
                    "RGB stock unavailable; prospective export requires initialized wallet stock"
                ),
            });
        }
        drop(stock);
        let mut candidate = psbt.clone();
        let (fascia, asset_beneficiaries) = self.color_psbt(&mut candidate, coloring_info)?;
        let runtime = self.rgb_runtime()?;
        let mut transfers = vec![];
        for (contract_id, beneficiaries) in asset_beneficiaries {
            let mut outputs = vec![];
            let mut secret_seals = vec![];
            for beneficiary in beneficiaries {
                match beneficiary {
                    BuilderSeal::Revealed(seal) => {
                        outputs.push(OutputSeal::with(fascia.witness_id(), seal.vout));
                    }
                    BuilderSeal::Concealed(seal) => secret_seals.push(seal),
                }
            }
            if outputs.is_empty() && secret_seals.is_empty() {
                return Err(Error::InvalidColoringInfo {
                    details: s!("no nonzero beneficiaries requested for transfer export"),
                });
            }
            transfers.push(runtime.transfer_from_fascia(
                contract_id,
                outputs,
                secret_seals,
                &fascia,
            )?);
        }
        *psbt = candidate;
        Ok(transfers)
    }

    /// Color a PSBT, consume the RGB fascia and return the related consignment.
    ///
    /// <div class="warning">This method is meant for special usage and is normally not needed, use
    /// it only if you know what you're doing</div>
    pub async fn color_psbt_and_consume(
        &self,
        psbt: &mut Psbt,
        coloring_info: ColoringInfo,
    ) -> Result<Vec<RgbTransfer>, Error> {
        let transfers = self.color_psbt_and_consume_inner(psbt, coloring_info)?;
        self.flush().await?;
        info!(self.logger, "Color PSBT and consume completed");
        Ok(transfers)
    }

    /// Color and consume using the established engine path, without persistence.
    ///
    /// The caller must validate immutable consent and every recipient BEFORE calling,
    /// then register outgoing rows and explicitly commit the complete snapshot. This
    /// method supplies neither authorization nor durable accounting. Any error may
    /// leave stock partially mutated: discard the isolated wallet, never retry it.
    /// The caller's PSBT is replaced only on success. No backup is scheduled.
    pub fn color_psbt_and_consume_in_memory(
        &self,
        psbt: &mut Psbt,
        coloring_info: ColoringInfo,
    ) -> Result<Vec<RgbTransfer>, Error> {
        if coloring_info.asset_info_map.is_empty() {
            return Err(Error::InvalidColoringInfo {
                details: s!("no assets requested for in-memory consumption"),
            });
        }
        let stock = self.rgb_stock.try_borrow().map_err(|_| Error::Internal {
            details: s!("RGB runtime already in use"),
        })?;
        if stock.is_none() {
            return Err(Error::Internal {
                details: s!("RGB stock unavailable for in-memory consumption"),
            });
        }
        drop(stock);
        let mut candidate = psbt.clone();
        let transfers = self.color_psbt_and_consume_inner(&mut candidate, coloring_info)?;
        *psbt = candidate;
        Ok(transfers)
    }

    fn color_psbt_and_consume_inner(
        &self,
        psbt: &mut Psbt,
        coloring_info: ColoringInfo,
    ) -> Result<Vec<RgbTransfer>, Error> {
        info!(self.logger, "Coloring PSBT and consuming...");
        let (fascia, asset_beneficiaries) = self.color_psbt(psbt, coloring_info.clone())?;

        let witness_txid = psbt.get_txid();

        let mut runtime = self.rgb_runtime()?;
        runtime.consume_fascia(fascia, None)?;

        let mut transfers = vec![];
        for (contract_id, beneficiaries) in asset_beneficiaries {
            let mut beneficiaries_witness = vec![];
            let mut beneficiaries_blinded = vec![];
            for builder_seal in beneficiaries {
                match builder_seal {
                    BuilderSeal::Revealed(seal) => {
                        let explicit_seal = ExplicitSeal::with(witness_txid, seal.vout);
                        beneficiaries_witness.push(explicit_seal);
                    }
                    BuilderSeal::Concealed(secret_seal) => {
                        beneficiaries_blinded.push(secret_seal);
                    }
                };
            }
            transfers.push(runtime.transfer(
                contract_id,
                beneficiaries_witness,
                beneficiaries_blinded,
                Some(witness_txid),
            )?);
        }
        drop(runtime);
        Ok(transfers)
    }

    /// Consume an RGB fascia.
    ///
    /// <div class="warning">This method is meant for special usage and is normally not needed, use
    /// it only if you know what you're doing</div>
    pub fn consume_fascia_in_memory(
        &self,
        fascia: Fascia,
        witness_ord: Option<WitnessOrd>,
    ) -> Result<(), Error> {
        let mut runtime = self.rgb_runtime()?;
        runtime.consume_fascia(fascia, witness_ord)?;
        Ok(())
    }

    /// Consume an RGB fascia and durably flush the updated stock.
    ///
    /// <div class="warning">This method is meant for special usage and is normally not needed, use
    /// it only if you know what you're doing</div>
    pub async fn consume_fascia(
        &self,
        fascia: Fascia,
        witness_ord: Option<WitnessOrd>,
    ) -> Result<(), Error> {
        info!(self.logger, "Consuming fascia...");
        self.consume_fascia_in_memory(fascia, witness_ord)?;
        self.flush().await?;
        info!(self.logger, "Consume fascia completed");
        Ok(())
    }

    /// Manually set the [`WitnessOrd`] of a witness TX.
    ///
    /// <div class="warning">This method is meant for special usage and is normally not needed, use
    /// it only if you know what you're doing</div>
    #[cfg(feature = "esplora")]
    pub fn upsert_witness(
        &self,
        witness_id: RgbTxid,
        witness_ord: WitnessOrd,
    ) -> Result<(), Error> {
        let mut runtime = self.rgb_runtime()?;
        runtime.upsert_witness(witness_id, witness_ord)?;
        Ok(())
    }

    /// Return `true` if a batch transfer with the given `txid` exists in the wallet database
    /// and is not in `Failed` status. Used to detect idempotent replay of `send_end` after a
    /// crash-inject reload: if the transfer is already present with a non-failed status, the
    /// broadcast already happened and the caller may treat the operation as succeeded.
    pub fn is_batch_transfer_sent(&self, txid: &str) -> Result<bool, Error> {
        let batch_transfers = self.database.iter_batch_transfers()?;
        Ok(batch_transfers
            .iter()
            .any(|bt| bt.txid.as_deref() == Some(txid) && bt.status != TransferStatus::Failed))
    }

    /// Return the current `Online` handle if the wallet has gone online, or `None` otherwise.
    pub fn get_online(&self) -> Option<Online> {
        self.online_data.as_ref().map(|od| Online {
            id: od.id,
            indexer_url: od.indexer_url.clone(),
        })
    }

    #[cfg(feature = "esplora")]
    pub(crate) fn save_new_asset_internal(
        &self,
        runtime: &RgbRuntime,
        contract_id: ContractId,
        asset_schema: AssetSchema,
        valid_contract: ValidContract,
        valid_transfer: ValidTransfer,
    ) -> Result<(), Error> {
        let timestamp = valid_contract.genesis.timestamp;
        let local_asset_data = match &asset_schema {
            AssetSchema::Nia => {
                let contract = runtime.contract_wrapper::<NonInflatableAsset>(contract_id)?;
                let spec = contract.spec();
                let ticker = spec.ticker().to_string();
                let name = spec.name().to_string();
                let details = spec.details().map(|d| d.to_string());
                let precision = spec.precision.into();
                let initial_supply = contract.total_issued_supply().into();
                let media_idx = if let Some(attachment) = contract.contract_terms().media {
                    Some(self.get_or_insert_media(
                        hex::encode(attachment.digest),
                        attachment.ty.to_string(),
                    )?)
                } else {
                    None
                };
                LocalAssetData {
                    name,
                    precision,
                    ticker: Some(ticker),
                    details,
                    media_idx,
                    initial_supply,
                    max_supply: None,
                    known_circulating_supply: None,
                    reject_list_url: None,
                }
            }
            AssetSchema::Ifa => {
                let contract = runtime.contract_wrapper::<InflatableFungibleAsset>(contract_id)?;
                let spec = contract.spec();
                let ticker = spec.ticker().to_string();
                let name = spec.name().to_string();
                let details = spec.details().map(|d| d.to_string());
                let precision = spec.precision.into();
                let media_idx = if let Some(attachment) = contract.contract_terms().media {
                    Some(self.get_or_insert_media(
                        hex::encode(attachment.digest),
                        attachment.ty.to_string(),
                    )?)
                } else {
                    None
                };
                let initial_supply = contract.total_issued_supply().into();
                let max_supply = contract.max_supply().into();
                let known_circulating_supply = IfaWrapper::with(valid_transfer.contract_data())
                    .total_issued_supply()
                    .into();
                let reject_list_url = contract.reject_list_url().map(|u| u.to_string());
                LocalAssetData {
                    name,
                    precision,
                    ticker: Some(ticker),
                    details,
                    media_idx,
                    initial_supply,
                    max_supply: Some(max_supply),
                    known_circulating_supply: Some(known_circulating_supply),
                    reject_list_url,
                }
            }
        };

        self.add_asset_to_db(
            contract_id.to_string(),
            &asset_schema,
            None,
            timestamp,
            local_asset_data,
        )?;

        Ok(())
    }

    /// Return the consignment file path for a send transfer of an asset.
    ///
    /// <div class="warning">This method is meant for special usage and is normally not needed, use
    /// it only if you know what you're doing</div>
    pub fn get_send_consignment_path(&self, asset_id: &str, transfer_id: &str) -> PathBuf {
        let transfer_dir = self.get_transfer_dir(transfer_id);
        let asset_transfer_dir = self.get_asset_transfer_dir(transfer_dir, asset_id);
        asset_transfer_dir.join(CONSIGNMENT_FILE)
    }

    /// Post a consignment to the proxy server.
    ///
    /// <div class="warning">This method is meant for special usage and is normally not needed, use
    /// it only if you know what you're doing</div>
    #[cfg(feature = "esplora")]
    pub async fn post_consignment(
        &self,
        proxy_url: &str,
        recipient_id: String,
        consignment_bytes: &[u8],
        txid: String,
        vout: Option<u32>,
    ) -> Result<(), Error> {
        info!(self.logger, "Posting consignment...");
        let consignment_res = self
            .wasm_proxy_client
            .post_consignment(
                proxy_url,
                recipient_id.clone(),
                consignment_bytes,
                txid.clone(),
                vout,
            )
            .await?;
        debug!(
            self.logger,
            "Consignment POST response: {:?}", consignment_res
        );

        if let Some(err) = consignment_res.error {
            if err.code == -101 {
                return Err(Error::RecipientIDAlreadyUsed);
            }
            return Err(Error::InvalidTransportEndpoint {
                details: format!("proxy error: {}", err.message),
            });
        }
        if consignment_res.result.is_none() {
            return Err(Error::InvalidTransportEndpoint {
                details: s!("invalid result"),
            });
        }

        info!(self.logger, "Post consignment completed");
        Ok(())
    }

    /// Get the height at which a transaction was mined.
    ///
    /// <div class="warning">This method is meant for special usage and is normally not needed, use
    /// it only if you know what you're doing</div>
    #[cfg(feature = "esplora")]
    pub async fn get_tx_height(&self, online: Online, txid: String) -> Result<Option<u32>, Error> {
        info!(self.logger, "Getting TX height...");
        self.check_online(online)?;
        let _ = RgbTxid::from_str(&txid).map_err(|_| Error::InvalidTxid)?;
        let height = self.indexer().get_tx_height(&txid).await?;
        info!(self.logger, "Get TX height completed");
        Ok(height)
    }

    /// Accept an RGB transfer by retrieving and validating its consignment from a proxy server.
    ///
    /// <div class="warning">This method is meant for special usage and is normally not needed, use
    /// it only if you know what you're doing</div>
    #[cfg(feature = "esplora")]
    pub async fn accept_transfer(
        &mut self,
        online: Online,
        txid: String,
        vout: u32,
        consignment_endpoint: RgbTransport,
        blinding: u64,
    ) -> Result<(RgbTransfer, Vec<Assignment>), Error> {
        info!(self.logger, "Accepting transfer...");
        self.check_online(online)?;
        let witness_id = RgbTxid::from_str(&txid).map_err(|_| Error::InvalidTxid)?;
        let proxy_url = TransportEndpoint::try_from(consignment_endpoint)?.endpoint;

        let consignment_res = self
            ._get_consignment_async(&proxy_url, txid.clone())
            .await?;
        let consignment_bytes = general_purpose::STANDARD
            .decode(consignment_res.consignment)
            .map_err(InternalError::from)?;
        let consignment = RgbTransfer::load(&consignment_bytes[..]).map_err(InternalError::from)?;

        let schema_id = consignment.schema_id().to_string();
        let asset_schema: AssetSchema = schema_id.try_into()?;
        self.check_schema_support(&asset_schema)?;
        debug!(
            self.logger,
            "Got consignment for asset with {} schema", asset_schema
        );

        let mut runtime = self.rgb_runtime()?;

        let graph_seal = GraphSeal::with_blinded_vout(vout, blinding);
        runtime.store_secret_seal(graph_seal)?;

        let wasm_resolver =
            crate::utils::WasmResolver::from_consignment(&consignment, self.chain_net());
        let resolver = crate::utils::OffchainResolverWasm {
            witness_id,
            consignment: &consignment,
            fallback: &wasm_resolver,
        };

        debug!(self.logger, "Validating consignment...");
        let asset_schema: AssetSchema = consignment.schema_id().try_into()?;
        let trusted_typesystem = asset_schema.types();
        let validation_config = ValidationConfig {
            chain_net: self.chain_net(),
            trusted_typesystem,
            ..Default::default()
        };
        let valid_consignment = match consignment.clone().validate(&resolver, &validation_config) {
            Ok(consignment) => consignment,
            Err(ValidationError::InvalidConsignment(e)) => {
                error!(self.logger, "Consignment is invalid: {}", e);
                return Err(Error::InvalidConsignment);
            }
            Err(ValidationError::ResolverError(e)) => {
                warn!(self.logger, "Network error during consignment validation");
                return Err(Error::Network {
                    details: e.to_string(),
                });
            }
        };
        let validity = valid_consignment.validation_status().validity();
        debug!(self.logger, "Consignment validity: {:?}", validity);

        let valid_contract = valid_consignment.clone().into_valid_contract();
        runtime
            .import_contract(valid_contract, &resolver)
            .expect("failure importing validated contract");

        let received_rgb_assignments =
            self.extract_received_assignments(&consignment, witness_id, Some(vout), None);

        runtime.accept_transfer(valid_consignment, &resolver)?;
        drop(runtime);
        self.flush().await?;

        info!(self.logger, "Accept transfer completed");
        Ok((
            consignment,
            received_rgb_assignments.into_values().collect(),
        ))
    }

    /// Update RGB witnesses.
    ///
    /// Pre-fetches witness data from esplora before calling the sync RGB stock method.
    ///
    /// <div class="warning">This method is meant for special usage and is normally not needed, use
    /// it only if you know what you're doing</div>
    #[cfg(feature = "esplora")]
    pub async fn update_witnesses(
        &mut self,
        online: Online,
        after_height: u32,
        force_witnesses: Vec<RgbTxid>,
    ) -> Result<UpdateRes, Error> {
        info!(self.logger, "Updating witnesses...");
        self.check_online(online)?;

        let mut cache = HashMap::new();
        for witness_id in &force_witnesses {
            let txid_str = witness_id.to_string();
            let txid = Txid::from_str(&txid_str).map_err(|_| Error::InvalidTxid)?;
            if let Some((tx, block_height, block_time)) =
                self.indexer().get_tx_with_status(&txid).await?
            {
                let witness_ord = match block_height.zip(block_time) {
                    Some((h, t)) => {
                        if let Some(height) = NonZeroU32::new(h) {
                            if let Some(pos) = WitnessPos::bitcoin(height, t as i64) {
                                WitnessOrd::Mined(pos)
                            } else {
                                WitnessOrd::Tentative
                            }
                        } else {
                            WitnessOrd::Tentative
                        }
                    }
                    None => WitnessOrd::Tentative,
                };
                cache.insert(*witness_id, WitnessStatus::Resolved(tx, witness_ord));
            }
        }

        let resolver = crate::utils::PreFetchResolver::new(cache, self.chain_net());
        let update_res =
            self.rgb_runtime()?
                .update_witnesses(&resolver, after_height, force_witnesses)?;

        info!(self.logger, "Update witnesses completed");
        Ok(update_res)
    }
}

/// Check whether the provided URL points to a valid proxy.
///
/// An error is raised if the provided proxy URL is invalid or if the service is running an
/// unsupported protocol version.
#[cfg(feature = "esplora")]
pub async fn check_proxy_url(proxy_url: &str) -> Result<(), Error> {
    crate::utils::check_proxy_async(proxy_url).await
}

/// Validate a consignment using the witness bundled in the consignment (offchain).
///
/// This works before the witness transaction is broadcast. The consignment bytes are
/// the raw strict-encoded consignment (not base64). The `txid` is the witness
/// transaction ID. The fallback resolver uses witness data from the consignment itself.
///
/// Returns a `ValidateConsignmentResult` with validity status, warnings, and error details.
#[cfg(feature = "esplora")]
pub fn validate_consignment_offchain(
    consignment_bytes: &[u8],
    txid: &str,
    bitcoin_network: BitcoinNetwork,
) -> Result<ValidateConsignmentResult, Error> {
    let consignment = RgbTransfer::load(consignment_bytes).map_err(|e| Error::Internal {
        details: format!("Failed to load consignment: {e}"),
    })?;

    let witness_id = RgbTxid::from_str(txid).map_err(|_| Error::InvalidTxid)?;
    let chain_net: ChainNet = bitcoin_network.into();
    let asset_schema: AssetSchema = consignment.schema_id().try_into()?;
    let trusted_typesystem = asset_schema.types();

    let wasm_resolver = crate::utils::WasmResolver::from_consignment(&consignment, chain_net);
    let resolver = crate::utils::OffchainResolverWasm {
        witness_id,
        consignment: &consignment,
        fallback: &wasm_resolver,
    };

    let validation_config = ValidationConfig {
        chain_net,
        trusted_typesystem,
        ..Default::default()
    };

    match consignment.clone().validate(&resolver, &validation_config) {
        Ok(valid_consignment) => {
            let status = valid_consignment.validation_status();
            Ok(ValidateConsignmentResult {
                valid: true,
                warnings: Some(
                    status
                        .warnings
                        .iter()
                        .map(|w| w.to_string())
                        .collect::<Vec<_>>(),
                ),
                error: None,
                details: None,
            })
        }
        Err(ValidationError::InvalidConsignment(failure)) => Ok(ValidateConsignmentResult {
            valid: false,
            warnings: None,
            error: Some("invalid".to_string()),
            details: Some(failure.to_string()),
        }),
        Err(ValidationError::ResolverError(e)) => Ok(ValidateConsignmentResult {
            valid: false,
            warnings: None,
            error: Some("resolver".to_string()),
            details: Some(e.to_string()),
        }),
    }
}

/// Facts derived only from validated immutable transfer bytes. Not spending authorization.
#[cfg(feature = "esplora")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InspectedTransfer {
    /// SHA256 of the complete canonical file bytes.
    pub bytes_sha256: String,
    /// Contract committed by the validated genesis.
    pub contract_id: String,
    /// Exact consensus-encoded colored witness transaction.
    pub witness_transaction: Vec<u8>,
    /// Witness transaction ID.
    pub witness_txid: String,
    /// Committed bundle ID.
    pub bundle_id: String,
    /// Validated anchor proof, bound by bytes_sha256.
    pub anchor_json: String,
    /// Single complete transition operation ID.
    pub transition_id: String,
    /// Every revealed fungible asset output, including change and fees.
    pub allocations: Vec<InspectedAllocation>,
}

/// A revealed allocation from the validated terminal transition.
#[cfg(feature = "esplora")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InspectedAllocation {
    /// Assignment index in the committed transition.
    pub assignment_index: u16,
    /// Witness output index.
    pub vout: u32,
    /// Exact integer RGB amount as decimal text (safe for JS).
    pub amount: String,
    /// Complete revealed seal, bound by bytes_sha256.
    pub seal: String,
    /// Output script from the exact colored witness transaction.
    pub script_pubkey: String,
}

/// Inspect a prospective, single-transition fungible transfer without accessing wallet state.
///
/// Validates the same canonical strict bytes with the established offchain validator.
/// Requires a complete witness exactly equal to the caller's colored transaction, a
/// fully revealed bundle and only nonzero fungible asset assignments. History may contain
/// other witnesses; only the requested witness's terminal bundle is inspected. Concealed,
/// missing, extra, duplicate or unsupported facts are refused. Returned facts grant no
/// ownership, chain authenticity, policy, consent or signing authority.
#[cfg(feature = "esplora")]
pub fn inspect_transfer_offchain(
    consignment_bytes: &[u8],
    colored_transaction: &[u8],
    bitcoin_network: BitcoinNetwork,
) -> Result<InspectedTransfer, Error> {
    let refuse = |details: &str| Error::InvalidColoringInfo {
        details: details.to_owned(),
    };
    if consignment_bytes.len() > 8_000_000 || colored_transaction.len() > 400_000 {
        return Err(refuse("inspection byte limit exceeded"));
    }
    let tx: bdk_wallet::bitcoin::Transaction =
        bdk_wallet::bitcoin::consensus::deserialize(colored_transaction)
            .map_err(|_| refuse("invalid colored transaction bytes"))?;
    let consignment = RgbTransfer::load(consignment_bytes).map_err(|e| Error::Internal {
        details: format!("Failed to load consignment: {e}"),
    })?;
    let mut canonical = vec![];
    consignment
        .save(&mut canonical)
        .map_err(InternalError::from)?;
    if canonical != consignment_bytes {
        return Err(refuse("noncanonical or trailing consignment bytes"));
    }
    let witness_id =
        RgbTxid::from_str(&tx.compute_txid().to_string()).map_err(|_| Error::InvalidTxid)?;
    let chain_net = bitcoin_network.into();
    let asset_schema: AssetSchema = consignment.schema_id().try_into()?;
    let fallback = crate::utils::WasmResolver::from_consignment(&consignment, chain_net);
    let resolver = crate::utils::OffchainResolverWasm {
        witness_id,
        consignment: &consignment,
        fallback: &fallback,
    };
    let config = ValidationConfig {
        chain_net,
        trusted_typesystem: asset_schema.types(),
        ..Default::default()
    };
    let valid = consignment
        .clone()
        .validate(&resolver, &config)
        .map_err(|_| refuse("consignment validation failed"))?;
    let matching: Vec<_> = valid
        .bundles
        .iter()
        .filter(|b| b.pub_witness.txid() == witness_id)
        .collect();
    if matching.len() != 1 {
        return Err(refuse("missing or ambiguous witness bundle"));
    }
    let witness = matching[0];
    let witness_bytes = inspect_exact_witness(&witness.pub_witness, colored_transaction)?;
    let bundle = &witness.bundle;
    if bundle.known_transitions.len() != 1
        || bundle.known_transitions_opids() != bundle.input_map_opids()
    {
        return Err(refuse("concealed or ambiguous terminal transitions"));
    }
    let known = &bundle.known_transitions[0];
    let transition = &known.transition;
    if transition.id() != known.opid
        || transition.contract_id != valid.contract_id()
        || transition.assignments.len() != 1
    {
        return Err(refuse("unsupported or extra transition assignments"));
    }
    let assigned = transition
        .assignments
        .get(&OS_ASSET)
        .ok_or_else(|| refuse("missing fungible asset assignments"))?;
    let allocations = inspect_revealed_allocations(assigned, &tx)?;
    // A prospective revealed-output export has no secret terminals. Refuse other modes.
    if !valid.terminals.is_empty() {
        return Err(refuse("unsupported secret terminal allocations"));
    }
    Ok(InspectedTransfer {
        bytes_sha256: <sha256::Hash as Sha256Hash>::hash(consignment_bytes).to_string(),
        contract_id: valid.contract_id().to_string(),
        witness_transaction: witness_bytes,
        witness_txid: witness_id.to_string(),
        bundle_id: bundle.bundle_id().to_string(),
        anchor_json: serde_json::to_string(&witness.anchor).map_err(InternalError::from)?,
        transition_id: known.opid.to_string(),
        allocations,
    })
}

#[cfg(feature = "esplora")]
fn inspect_exact_witness(witness: &PubWitness, colored: &[u8]) -> Result<Vec<u8>, Error> {
    let refuse = |details: &str| Error::InvalidColoringInfo {
        details: details.to_owned(),
    };
    let tx = witness
        .tx()
        .ok_or_else(|| refuse("concealed witness transaction"))?;
    let bytes = bdk_wallet::bitcoin::consensus::serialize(tx);
    if bytes != colored {
        return Err(refuse("colored transaction substitution"));
    }
    Ok(bytes)
}

#[cfg(feature = "esplora")]
fn inspect_revealed_allocations(
    assigned: &rgbstd::TypedAssigns<GraphSeal>,
    tx: &bdk_wallet::bitcoin::Transaction,
) -> Result<Vec<InspectedAllocation>, Error> {
    let refuse = |details: &str| Error::InvalidColoringInfo {
        details: details.to_owned(),
    };
    if !matches!(assigned, rgbstd::TypedAssigns::Fungible(_)) {
        return Err(refuse("unsupported non-fungible assignments"));
    }
    let mut allocations = vec![];
    let mut vouts = HashSet::new();
    for (index, assignment) in assigned.as_fungible().iter().enumerate() {
        let (seal, amount) = assignment
            .as_revealed()
            .ok_or_else(|| refuse("concealed recipient seal"))?;
        if !matches!(seal.txid, rgbstd::txout::TxPtr::WitnessTx) {
            return Err(refuse("recipient seal targets another transaction"));
        }
        let vout = seal.vout.to_u32();
        let output = tx
            .output
            .get(vout as usize)
            .ok_or_else(|| refuse("recipient vout outside colored transaction"))?;
        if amount.as_u64() == 0 || !vouts.insert(vout) || output.script_pubkey.is_op_return() {
            return Err(refuse(
                "zero, duplicate or unspendable recipient allocation",
            ));
        }
        allocations.push(InspectedAllocation {
            assignment_index: index as u16,
            vout,
            amount: amount.as_u64().to_string(),
            seal: seal.to_string(),
            script_pubkey: output.script_pubkey.to_hex_string(),
        });
    }
    if allocations.is_empty() {
        return Err(refuse("missing recipient allocations"));
    }
    Ok(allocations)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inspection_transaction() -> Vec<u8> {
        bdk_wallet::bitcoin::consensus::serialize(&bdk_wallet::bitcoin::Transaction {
            version: bdk_wallet::bitcoin::transaction::Version::TWO,
            lock_time: bdk_wallet::bitcoin::absolute::LockTime::ZERO,
            input: vec![bdk_wallet::bitcoin::TxIn::default()],
            output: vec![TxOut {
                value: BdkAmount::ZERO,
                script_pubkey: ScriptBuf::new(),
            }],
        })
    }

    #[test]
    fn inspect_transfer_exact_witness_substitution() {
        let bytes = inspection_transaction();
        let tx: bdk_wallet::bitcoin::Transaction =
            bdk_wallet::bitcoin::consensus::deserialize(&bytes).unwrap();
        let witness = PubWitness::Tx(tx.clone());
        assert_eq!(inspect_exact_witness(&witness, &bytes).unwrap(), bytes);
        let mut other = tx.clone();
        other.output[0].value = BdkAmount::from_sat(1);
        assert!(
            inspect_exact_witness(&witness, &bdk_wallet::bitcoin::consensus::serialize(&other))
                .is_err()
        );
        // Txid equality alone does not bind witness data: compare full consensus bytes.
        other = tx;
        other.input[0].witness.push([1]);
        assert_eq!(other.compute_txid().to_string(), witness.txid().to_string());
        assert!(
            inspect_exact_witness(&witness, &bdk_wallet::bitcoin::consensus::serialize(&other))
                .is_err()
        );
        assert!(inspect_exact_witness(&PubWitness::Txid(witness.txid()), &bytes).is_err());
    }

    #[test]
    fn inspect_transfer_allocation_structural_refusals() {
        use rgbstd::{RevealedValue, TypedAssigns};
        use strict_encoding::StrictDumb;
        // Structural vectors only: no valid RGB consignment or funded fixture is claimed.
        let tx: bdk_wallet::bitcoin::Transaction =
            bdk_wallet::bitcoin::consensus::deserialize(&inspection_transaction()).unwrap();
        let seal = GraphSeal::strict_dumb();
        let make = |assignments: Vec<Assign<RevealedValue, GraphSeal>>| {
            serde_json::from_value::<TypedAssigns<GraphSeal>>(
                serde_json::json!({"type": "fungible", "items": assignments}),
            )
            .unwrap()
        };
        let mut seal = seal;
        seal.txid = rgbstd::txout::TxPtr::WitnessTx;
        seal.vout = 0.into();
        let revealed = Assign::revealed(seal, RevealedValue::from(1));
        let facts = inspect_revealed_allocations(&make(vec![revealed.clone()]), &tx).unwrap();
        assert_eq!(facts.len(), 1);
        assert_eq!(facts[0].amount, "1");
        assert_eq!(facts[0].vout, 0);
        assert!(
            inspect_revealed_allocations(&make(vec![revealed.clone(), revealed]), &tx).is_err()
        );
        assert!(
            inspect_revealed_allocations(&make(vec![Assign::revealed(seal, 0.into())]), &tx)
                .is_err()
        );
        assert!(
            inspect_revealed_allocations(
                &make(vec![Assign::ConfidentialSeal {
                    seal: seal.conceal(),
                    state: 1.into(),
                }]),
                &tx
            )
            .is_err()
        );
        seal.vout = 1.into();
        assert!(
            inspect_revealed_allocations(&make(vec![Assign::revealed(seal, 1.into())]), &tx)
                .is_err()
        );
        seal.vout = 0.into();
        seal.txid = rgbstd::txout::TxPtr::Txid(RgbTxid::strict_dumb());
        assert!(
            inspect_revealed_allocations(&make(vec![Assign::revealed(seal, 1.into())]), &tx)
                .is_err()
        );
        seal.txid = rgbstd::txout::TxPtr::WitnessTx;
        let mut opreturn = tx;
        opreturn.output[0].script_pubkey = ScriptBuf::new_op_return([]);
        assert!(
            inspect_revealed_allocations(&make(vec![Assign::revealed(seal, 1.into())]), &opreturn)
                .is_err()
        );
        let unsupported: TypedAssigns<GraphSeal> = TypedAssigns::strict_dumb();
        assert!(inspect_revealed_allocations(&unsupported, &opreturn).is_err());
    }

    #[test]
    fn inspect_transfer_malformed_transaction_and_consignment() {
        assert!(matches!(
            inspect_transfer_offchain(b"bad", b"bad", BitcoinNetwork::Regtest),
            Err(Error::InvalidColoringInfo { .. })
        ));
        assert!(matches!(
            inspect_transfer_offchain(b"bad", &inspection_transaction(), BitcoinNetwork::Regtest),
            Err(Error::Internal { .. })
        ));
    }

    #[test]
    fn inspect_transfer_rejects_trailing_bytes_before_schema_validation() {
        use strict_encoding::StrictDumb;
        let transfer = RgbTransfer::strict_dumb();
        let mut bytes = vec![];
        transfer.save(&mut bytes).unwrap();
        bytes.push(42);
        assert!(
            matches!(inspect_transfer_offchain(&bytes, &inspection_transaction(), BitcoinNetwork::Regtest),
            Err(Error::InvalidColoringInfo { details }) if details.contains("trailing"))
        );
        bytes.pop();
        // Strict-decodable synthetic data is not validated RGB evidence.
        assert!(
            inspect_transfer_offchain(&bytes, &inspection_transaction(), BitcoinNetwork::Regtest)
                .is_err()
        );
    }

    #[test]
    fn validate_consignment_offchain_invalid_bytes() {
        let result = validate_consignment_offchain(
            b"not a valid consignment",
            "0000000000000000000000000000000000000000000000000000000000000000",
            BitcoinNetwork::Regtest,
        );
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(err, Error::Internal { details: ref d } if d.contains("Failed to load consignment")),
            "expected Internal error with load failure, got: {err:?}"
        );
    }

    #[test]
    fn validate_consignment_offchain_invalid_txid() {
        let result = validate_consignment_offchain(
            b"not a valid consignment",
            "not-a-txid",
            BitcoinNetwork::Regtest,
        );
        // Will fail on consignment load before txid parsing
        assert!(result.is_err());
    }

    #[test]
    fn validate_consignment_result_serde_roundtrip() {
        let result = ValidateConsignmentResult {
            valid: true,
            warnings: Some(vec!["warn1".to_string()]),
            error: None,
            details: None,
        };
        let json = serde_json::to_string(&result).unwrap();
        let deserialized: ValidateConsignmentResult = serde_json::from_str(&json).unwrap();
        assert!(deserialized.valid);
        assert_eq!(deserialized.warnings.unwrap(), vec!["warn1".to_string()]);
        assert!(deserialized.error.is_none());

        let result_invalid = ValidateConsignmentResult {
            valid: false,
            warnings: None,
            error: Some("invalid".to_string()),
            details: Some("schema mismatch".to_string()),
        };
        let json = serde_json::to_string(&result_invalid).unwrap();
        let deserialized: ValidateConsignmentResult = serde_json::from_str(&json).unwrap();
        assert!(!deserialized.valid);
        assert_eq!(deserialized.error.unwrap(), "invalid");
    }

    #[test]
    fn indexer_protocol_display() {
        assert_eq!(IndexerProtocol::Esplora.to_string(), "Esplora");
    }
}
