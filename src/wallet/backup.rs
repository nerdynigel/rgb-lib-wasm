//! WASM backup: encrypt/decrypt wallet state to/from bytes.

use amplify::s;
use base64::{Engine as _, engine::general_purpose};
use chacha20poly1305::{KeyInit, XChaCha20Poly1305, aead::Aead};
use generic_array::GenericArray;
use scrypt::{
    Scrypt,
    password_hash::{PasswordHasher, Salt, SaltString, rand_core::OsRng},
};
use serde::{Deserialize, Serialize};

use crate::Error;
use crate::error::InternalError;

pub(crate) const BACKUP_VERSION: u8 = 2;
/// 24-byte nonce for XChaCha20Poly1305 (single-shot mode).
const NONCE_LEN: usize = 24;

/// Public (unencrypted) metadata stored alongside the encrypted payload.
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct BackupPubData {
    pub(crate) salt: String,
    pub(crate) nonce: String, // hex-encoded 24-byte nonce
    pub(crate) version: u8,
}

/// The serializable wallet state that gets encrypted.
#[derive(Serialize, Deserialize)]
pub(crate) struct WalletBackupPayload {
    /// Identity binds the encrypted state to the wallet that may restore it.
    /// Older backups without this field cannot be used for external-L1 recovery.
    #[serde(default)]
    pub(crate) wallet_identity: Option<(String, String, String)>,
    pub(crate) db: crate::database::memory_db::InMemoryDb,
    pub(crate) bdk_changeset: Option<bdk_wallet::ChangeSet>,
    /// Strict-encoded MemStash, base64
    pub(crate) stock_stash_b64: String,
    /// Strict-encoded MemState, base64
    pub(crate) stock_state_b64: String,
    /// Strict-encoded MemIndex, base64
    pub(crate) stock_index_b64: String,
    /// Pinned derivation index per keychain for address reuse.
    #[serde(default)]
    pub(crate) reuse_address_index: std::collections::HashMap<bdk_wallet::KeychainKind, u32>,
    #[serde(default)]
    pub(crate) transfer_artifacts:
        std::collections::HashMap<String, super::online::TransferArtifacts>,
    #[serde(default)]
    pub(crate) received_consignments: std::collections::HashMap<String, Vec<u8>>,
}

/// Derive a 32-byte key from password + salt using Scrypt.
pub(crate) fn derive_key(
    password: &str,
    salt_b64: &str,
) -> Result<GenericArray<u8, typenum::U32>, Error> {
    let salt = Salt::from_b64(salt_b64).map_err(InternalError::from)?;
    let password_hash = Scrypt
        .hash_password_customized(
            password.as_bytes(),
            None,
            None,
            scrypt::Params::default(),
            salt,
        )
        .map_err(InternalError::from)?;
    let hash_output = password_hash
        .hash
        .ok_or(InternalError::NoPasswordHashError)?;
    Ok(*GenericArray::from_slice(hash_output.as_bytes()))
}

/// Encrypt plaintext bytes using XChaCha20Poly1305.
pub(crate) fn encrypt_payload(
    plaintext: &[u8],
    password: &str,
) -> Result<(Vec<u8>, BackupPubData), Error> {
    let salt = SaltString::generate(&mut OsRng);
    let key = derive_key(password, salt.as_str())?;

    let mut nonce_bytes = [0u8; NONCE_LEN];
    getrandom::getrandom(&mut nonce_bytes).map_err(|e| Error::Internal {
        details: format!("RNG unavailable: {e}"),
    })?;
    let nonce = GenericArray::from_slice(&nonce_bytes);

    let cipher = XChaCha20Poly1305::new(&key);
    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| InternalError::AeadError(e.to_string()))?;

    let pub_data = BackupPubData {
        salt: salt.to_string(),
        nonce: hex::encode(nonce_bytes),
        version: BACKUP_VERSION,
    };
    Ok((ciphertext, pub_data))
}

/// Decrypt ciphertext bytes using XChaCha20Poly1305.
pub(crate) fn decrypt_payload(
    ciphertext: &[u8],
    password: &str,
    pub_data: &BackupPubData,
) -> Result<Vec<u8>, Error> {
    if pub_data.version != BACKUP_VERSION {
        return Err(Error::UnsupportedBackupVersion {
            version: pub_data.version.to_string(),
        });
    }
    let key = derive_key(password, &pub_data.salt)?;
    let nonce_bytes =
        hex::decode(&pub_data.nonce).map_err(|e| InternalError::HexError(e.to_string()))?;
    if nonce_bytes.len() != NONCE_LEN {
        return Err(Error::InvalidBackup);
    }
    let nonce = GenericArray::from_slice(&nonce_bytes);

    let cipher = XChaCha20Poly1305::new(&key);
    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| Error::WrongPassword)
}

/// Encode: `[4-byte LE pub_data_len][pub_data JSON][encrypted payload]`
pub(crate) fn encode_backup(ciphertext: &[u8], pub_data: &BackupPubData) -> Result<Vec<u8>, Error> {
    let pub_json = serde_json::to_vec(pub_data).map_err(InternalError::from)?;
    let pub_len = (pub_json.len() as u32).to_le_bytes();
    let mut out = Vec::with_capacity(4 + pub_json.len() + ciphertext.len());
    out.extend_from_slice(&pub_len);
    out.extend_from_slice(&pub_json);
    out.extend_from_slice(ciphertext);
    Ok(out)
}

/// Decode: split backup bytes into (BackupPubData, ciphertext).
pub(crate) fn decode_backup(data: &[u8]) -> Result<(BackupPubData, &[u8]), Error> {
    if data.len() < 4 {
        return Err(Error::InvalidBackup);
    }
    let pub_len = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
    let end = 4usize.checked_add(pub_len).ok_or(Error::InvalidBackup)?;
    if pub_len > 4096 || data.len() < end || data.len() > 16_777_216 {
        return Err(Error::InvalidBackup);
    }
    let pub_data: BackupPubData =
        serde_json::from_slice(&data[4..end]).map_err(|_| Error::InvalidBackup)?;
    let ciphertext = &data[end..];
    Ok((pub_data, ciphertext))
}

use strict_encoding::StrictSerialize;

impl super::Wallet {
    /// Read-only exact-state check for a lost restore acknowledgement. Only
    /// backup bookkeeping timestamps/counters are excluded; every financial
    /// record, BDK changeset, Stock byte and pending artifact must match.
    pub fn matches_backup_state(&self, bytes: &[u8], password: &str) -> Result<bool, Error> {
        let (public, ciphertext) = decode_backup(bytes)?;
        let plaintext = decrypt_payload(ciphertext, password, &public)?;
        let mut payload: WalletBackupPayload =
            serde_json::from_slice(&plaintext).map_err(|_| Error::InvalidBackup)?;
        self.check_backup_identity(&payload)?;
        // Normal unlock reveals the next public address. It must not make a
        // completed restore look like a different financial history. Permit
        // only monotonic public derivation/cache growth and keep the live BDK
        // state intact; transactions, chain, descriptors and network must match.
        if let (Some(expected), Some(actual)) =
            (&mut payload.bdk_changeset, self.bdk_database.get_data())
        {
            if !derivation_covers(&actual.indexer, &expected.indexer) {
                return Ok(false);
            }
            expected.indexer = actual.indexer.clone();
        }
        let mut expected = serde_json::to_value(payload).map_err(InternalError::from)?;
        let mut actual: serde_json::Value =
            serde_json::from_slice(&self.serialize_backup_payload()?)
                .map_err(InternalError::from)?;
        for value in [&mut expected, &mut actual] {
            let db = value
                .get_mut("db")
                .and_then(|v| v.as_object_mut())
                .ok_or(Error::InvalidBackup)?;
            db.remove("backup_info");
            db.remove("next_backup_info_idx");
        }
        Ok(expected == actual)
    }
    /// Create an encrypted backup of the wallet state.
    /// Returns the backup as bytes (password-protected).
    pub fn backup(&self, password: &str) -> Result<Vec<u8>, Error> {
        let payload_json = self.serialize_backup_payload()?;

        let (ciphertext, pub_data) = encrypt_payload(&payload_json, password)?;
        let backup_bytes = encode_backup(&ciphertext, &pub_data)?;

        self.update_backup_info(true)?;

        Ok(backup_bytes)
    }

    /// Restore wallet state from an encrypted backup.
    /// The wallet must be created first via `Wallet::new()` with the same mnemonic/xpubs.
    pub fn restore_backup(&mut self, backup_bytes: &[u8], password: &str) -> Result<(), Error> {
        // 1. Decode envelope
        let (pub_data, ciphertext) = decode_backup(backup_bytes)?;

        // 2. Decrypt
        let payload_json = decrypt_payload(ciphertext, password, &pub_data)?;

        // 3. Deserialize payload
        let payload: WalletBackupPayload =
            serde_json::from_slice(&payload_json).map_err(|_| Error::InvalidBackup)?;
        self.check_backup_identity(&payload)?;

        self.install_backup_payload(payload)?;
        self.trigger_auto_backup();
        Ok(())
    }

    // Decode and validate the entire snapshot in a temporary instance. Invalid
    // Stock/BDK data must never leave the live wallet half-restored.
    fn install_backup_payload(&mut self, payload: WalletBackupPayload) -> Result<(), Error> {
        self.check_backup_identity(&payload)?;
        // Device recovery is permitted only into an empty engine. Never
        // overwrite newer financial state or discard unresolved obligations.
        let current = self.database.get_db_data(false)?;
        if !current.batch_transfers.is_empty() || !current.txos.is_empty() {
            return Err(Error::InvalidBackup);
        }
        let snapshot = super::idb_store::WalletSnapshot {
            sequence: 0,
            db: payload.db,
            bdk_changeset: payload.bdk_changeset,
            transfer_artifacts: payload.transfer_artifacts,
            received_consignments: payload.received_consignments,
            stock_stash_b64: Some(payload.stock_stash_b64),
            stock_state_b64: Some(payload.stock_state_b64),
            stock_index_b64: Some(payload.stock_index_b64),
            reuse_address_index: payload.reuse_address_index,
        };
        let mut candidate = Self::new(self.wallet_data.clone())?;
        candidate.restore_from_snapshot(snapshot)?;
        candidate.idb_sequence = self.idb_sequence.clone();
        candidate.vss_client = self.vss_client.take();
        *self = candidate;
        Ok(())
    }

    /// Returns true if the wallet has been modified since the last backup.
    pub fn backup_info(&self) -> Result<bool, Error> {
        let info = self.database.get_backup_info()?;
        match info {
            None => Ok(true), // never backed up
            Some(info) => {
                if info.last_backup_timestamp.is_empty() {
                    Ok(true)
                } else {
                    Ok(info.last_operation_timestamp > info.last_backup_timestamp)
                }
            }
        }
    }
}

fn derivation_covers(
    actual: &bdk_wallet::chain::indexer::keychain_txout::ChangeSet,
    expected: &bdk_wallet::chain::indexer::keychain_txout::ChangeSet,
) -> bool {
    expected.last_revealed.iter().all(|(id, index)| {
        actual
            .last_revealed
            .get(id)
            .is_some_and(|current| current >= index)
    }) && expected.spk_cache.iter().all(|(id, cache)| {
        cache.iter().all(|(index, script)| {
            actual
                .spk_cache
                .get(id)
                .and_then(|current| current.get(index))
                == Some(script)
        })
    })
}

use super::vss::{VssBackupClient, VssBackupConfig, VssBackupInfo};

impl super::Wallet {
    /// Configure VSS backup for this wallet.
    pub fn configure_vss_backup(&mut self, config: &VssBackupConfig) {
        self.vss_client = Some(VssBackupClient::new(config));
    }

    /// Disable VSS backup.
    pub fn disable_vss_backup(&mut self) {
        self.vss_client = None;
    }

    /// Upload an encrypted backup to the configured VSS server.
    /// The wallet must have a VSS client configured via configure_vss_backup().
    pub async fn vss_backup(&self) -> Result<i64, Error> {
        let client = self.vss_client.as_ref().ok_or(Error::Internal {
            details: s!("VSS backup not configured"),
        })?;

        let payload_json = self.serialize_backup_payload()?;
        let fingerprint = self.wallet_data.master_fingerprint.clone();

        self.update_backup_info(true)?;
        match client.upload_backup(&payload_json, &fingerprint).await {
            Ok(version) => {
                *self.last_vss_backup_error.borrow_mut() = None;
                Ok(version)
            }
            Err(e) => {
                let _ = self.update_backup_info(false);
                *self.last_vss_backup_error.borrow_mut() = Some(e.to_string());
                Err(e)
            }
        }
    }

    /// Upload a backup if the wallet has changes not yet backed up. Call after
    /// `configure_vss_backup` so enabling backups never leaves an empty store.
    /// Returns the new server version, or `None` when no upload was needed.
    pub async fn ensure_initial_backup(&self) -> Result<Option<i64>, Error> {
        if self.vss_client.is_none() {
            return Err(Error::Internal {
                details: s!("VSS backup not configured"),
            });
        }
        if !self.backup_info()? {
            return Ok(None);
        }
        self.vss_backup().await.map(Some)
    }

    /// Download and restore wallet state from VSS server.
    pub async fn vss_restore_backup(&mut self) -> Result<(), Error> {
        let client = self.vss_client.as_ref().ok_or(Error::Internal {
            details: s!("VSS backup not configured"),
        })?;

        let payload_json = client.download_backup().await?;
        let payload: WalletBackupPayload =
            serde_json::from_slice(&payload_json).map_err(|_| Error::InvalidBackup)?;
        self.check_backup_identity(&payload)?;

        self.install_backup_payload(payload)?;
        // Acknowledgement is conditional on the actual IDB commit.
        self.flush().await
    }

    /// Query VSS backup status.
    pub async fn vss_backup_info(&self) -> Result<VssBackupInfo, Error> {
        let client = self.vss_client.as_ref().ok_or(Error::Internal {
            details: s!("VSS backup not configured"),
        })?;
        let server_version = client.get_backup_version().await?;
        let backup_required = self.backup_info()?;

        Ok(VssBackupInfo {
            backup_exists: server_version.is_some(),
            server_version,
            backup_required,
            last_backup_error: self.last_vss_backup_error.borrow().clone(),
        })
    }

    /// Serialize the wallet state to JSON bytes (shared by local backup and VSS).
    fn serialize_backup_payload(&self) -> Result<Vec<u8>, Error> {
        let stock_ref = self.rgb_stock.borrow();
        let stock = stock_ref.as_ref().ok_or_else(|| Error::Internal {
            details: s!("RGB stock is currently in use (runtime active)"),
        })?;

        let stash_bytes = stock
            .as_stash_provider()
            .to_strict_serialized::<{ u32::MAX as usize }>()
            .map_err(|e| InternalError::StockError(e.to_string()))?;
        let state_bytes = stock
            .as_state_provider()
            .to_strict_serialized::<{ u32::MAX as usize }>()
            .map_err(|e| InternalError::StockError(e.to_string()))?;
        let index_bytes = stock
            .as_index_provider()
            .to_strict_serialized::<{ u32::MAX as usize }>()
            .map_err(|e| InternalError::StockError(e.to_string()))?;

        drop(stock_ref);

        let payload = WalletBackupPayload {
            wallet_identity: Some(self.backup_identity()),
            db: self.database.as_ref().clone(),
            bdk_changeset: self.bdk_database.get_data().clone(),
            stock_stash_b64: general_purpose::STANDARD.encode(stash_bytes.as_unconfined()),
            stock_state_b64: general_purpose::STANDARD.encode(state_bytes.as_unconfined()),
            stock_index_b64: general_purpose::STANDARD.encode(index_bytes.as_unconfined()),
            reuse_address_index: self.reuse_address_index.clone(),
            transfer_artifacts: self.transfer_artifacts.clone(),
            received_consignments: self.received_consignments.clone(),
        };

        serde_json::to_vec(&payload).map_err(|e| InternalError::from(e).into())
    }

    fn backup_identity(&self) -> (String, String, String) {
        (
            self.wallet_data.bitcoin_network.to_string(),
            self.wallet_data.account_xpub_vanilla.clone(),
            self.wallet_data.account_xpub_colored.clone(),
        )
    }

    fn check_backup_identity(&self, payload: &WalletBackupPayload) -> Result<(), Error> {
        if payload.wallet_identity.as_ref() != Some(&self.backup_identity()) {
            return Err(Error::InvalidBackup);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restore_comparison_only_allows_monotonic_public_derivation_growth() {
        use bdk_wallet::chain::{DescriptorId, indexer::keychain_txout::ChangeSet};
        let id: DescriptorId = "11".repeat(32).parse().unwrap();
        let mut expected = ChangeSet::default();
        expected.last_revealed.insert(id, 1);
        expected
            .spk_cache
            .entry(id)
            .or_default()
            .insert(0, crate::bitcoin::ScriptBuf::from_bytes(vec![0x51]));
        let mut actual = expected.clone();
        actual.last_revealed.insert(id, 2);
        assert!(derivation_covers(&actual, &expected));
        actual.last_revealed.insert(id, 0);
        assert!(!derivation_covers(&actual, &expected));
        actual.last_revealed.insert(id, 2);
        actual
            .spk_cache
            .get_mut(&id)
            .unwrap()
            .insert(0, crate::bitcoin::ScriptBuf::from_bytes(vec![0x52]));
        assert!(!derivation_covers(&actual, &expected));
    }

    #[test]
    fn oversized_envelope_length_is_refused() {
        assert!(matches!(
            decode_backup(&[255, 255, 255, 255, 0]),
            Err(Error::InvalidBackup)
        ));
    }

    #[test]
    fn backup_payload_preserves_pending_external_and_received_bytes() {
        use super::super::online::{ExternalSendArtifact, ExternalSendStatus, TransferArtifacts};
        let artifacts = TransferArtifacts {
            consignment_bytes: std::collections::HashMap::from([("asset".into(), vec![1, 2, 3])]),
            external_send: Some(ExternalSendArtifact {
                prepared_psbt: "exact-colored-psbt".into(),
                expected_tx_hex: "010203".into(),
                fascia_json: "{}".into(),
                recipients_digest: "bound-recipients".into(),
                status: ExternalSendStatus::BroadcastRecorded,
            }),
            ..Default::default()
        };
        let payload = WalletBackupPayload {
            wallet_identity: Some((
                "regtest".into(),
                "vanilla-xpub".into(),
                "colored-xpub".into(),
            )),
            db: crate::database::memory_db::InMemoryDb::new(),
            bdk_changeset: None,
            stock_stash_b64: "stash".into(),
            stock_state_b64: "state".into(),
            stock_index_b64: "index".into(),
            reuse_address_index: Default::default(),
            transfer_artifacts: std::collections::HashMap::from([(
                "transaction".into(),
                artifacts,
            )]),
            received_consignments: std::collections::HashMap::from([(
                "recipient".into(),
                vec![4, 5, 6],
            )]),
        };
        let encoded = serde_json::to_vec(&payload).unwrap();
        let restored: WalletBackupPayload = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(restored.wallet_identity, payload.wallet_identity);
        let pending = &restored.transfer_artifacts["transaction"];
        assert_eq!(
            pending.external_send.as_ref().unwrap().status,
            ExternalSendStatus::BroadcastRecorded
        );
        assert_eq!(pending.consignment_bytes["asset"], vec![1, 2, 3]);
        assert_eq!(restored.received_consignments["recipient"], vec![4, 5, 6]);
    }

    #[test]
    fn malformed_nonce_is_refused_without_panic() {
        let pub_data = BackupPubData {
            salt: "dGVzdHNhbHQ".into(),
            nonce: "00".into(),
            version: BACKUP_VERSION,
        };
        assert!(matches!(
            decrypt_payload(b"ciphertext", "test-password", &pub_data),
            Err(Error::InvalidBackup)
        ));
    }

    #[test]
    fn encrypt_decrypt_round_trip() {
        let plaintext = b"hello, RGB wallet backup!";
        let password = "strong_password_42";

        let (ciphertext, pub_data) = encrypt_payload(plaintext, password).unwrap();
        assert_ne!(ciphertext, plaintext);
        assert_eq!(pub_data.version, BACKUP_VERSION);

        let decrypted = decrypt_payload(&ciphertext, password, &pub_data).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn wrong_password_fails() {
        let plaintext = b"secret data";
        let (ciphertext, pub_data) = encrypt_payload(plaintext, "correct").unwrap();

        let result = decrypt_payload(&ciphertext, "wrong", &pub_data);
        assert!(matches!(result, Err(Error::WrongPassword)));
    }

    #[test]
    fn unsupported_version_fails() {
        let plaintext = b"data";
        let (ciphertext, mut pub_data) = encrypt_payload(plaintext, "pw").unwrap();
        pub_data.version = 99;

        let result = decrypt_payload(&ciphertext, "pw", &pub_data);
        assert!(matches!(
            result,
            Err(Error::UnsupportedBackupVersion { .. })
        ));
    }

    #[test]
    fn encode_decode_round_trip() {
        let ciphertext = b"encrypted_blob_here";
        let pub_data = BackupPubData {
            salt: "dGVzdHNhbHQ".to_string(),
            nonce: hex::encode([0xABu8; NONCE_LEN]),
            version: BACKUP_VERSION,
        };

        let encoded = encode_backup(ciphertext, &pub_data).unwrap();
        let (decoded_pub, decoded_ct) = decode_backup(&encoded).unwrap();

        assert_eq!(decoded_ct, ciphertext);
        assert_eq!(decoded_pub.salt, pub_data.salt);
        assert_eq!(decoded_pub.nonce, pub_data.nonce);
        assert_eq!(decoded_pub.version, BACKUP_VERSION);
    }

    #[test]
    fn decode_too_short_fails() {
        assert!(matches!(decode_backup(&[0, 1]), Err(Error::InvalidBackup)));
        assert!(matches!(decode_backup(&[]), Err(Error::InvalidBackup)));
    }

    #[test]
    fn decode_truncated_pub_data_fails() {
        // pub_len says 100 but only 5 bytes follow
        let data = [100u8, 0, 0, 0, 0, 0, 0, 0, 0];
        assert!(matches!(decode_backup(&data), Err(Error::InvalidBackup)));
    }

    #[test]
    fn derive_key_deterministic() {
        let salt = SaltString::generate(&mut OsRng);
        let k1 = derive_key("password", salt.as_str()).unwrap();
        let k2 = derive_key("password", salt.as_str()).unwrap();
        assert_eq!(k1, k2);

        let k3 = derive_key("different", salt.as_str()).unwrap();
        assert_ne!(k1, k3);
    }
}
