// Test-only byte measurements. Never expose wallet bytes/keys in receipts.
use base64::{Engine as _, engine::general_purpose::STANDARD};
use sha2::{Digest, Sha256};

pub fn strict_stock_bytes(parts: &(String, String, String)) -> [Vec<u8>; 3] {
    [&parts.0, &parts.1, &parts.2].map(|part| STANDARD.decode(part).unwrap())
}

pub fn snapshot_stock_bytes(value: &serde_json::Value) -> [Vec<u8>; 3] {
    ["stock_stash_b64", "stock_state_b64", "stock_index_b64"]
        .map(|field| STANDARD.decode(value[field].as_str().unwrap()).unwrap())
}

// JSON object order is not state; only sequence is deliberately excluded.
// This is distinct from the exact persisted UTF-8 bytes measured separately.
pub fn snapshot_state_bytes(mut value: serde_json::Value) -> Vec<u8> {
    value.as_object_mut().unwrap().remove("sequence");
    value.sort_all_objects();
    serde_json::to_vec(&value).unwrap()
}

pub fn receipt(bytes: &[u8]) -> serde_json::Value {
    serde_json::json!({"bytes": bytes.len(), "sha256": hex::encode(Sha256::digest(bytes))})
}
