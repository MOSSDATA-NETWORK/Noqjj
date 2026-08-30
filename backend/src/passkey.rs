/// Passkey / WebAuthn 支持（预留，后续完善）
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct StoredCredential {
    pub credential_id: Vec<u8>,
    pub credential: serde_json::Value,
}
