use keyring::Entry;
use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, AeadCore, KeyInit, OsRng}
};
use serde::{Serialize, Deserialize};
use rand::RngCore;

fn get_or_create_master_key() -> Result<String, String> {
    let entry = Entry::new("com.io.kairo", "user").map_err(|e| e.to_string())?;
    match entry.get_password() {
        Ok(password) => Ok(password),
        Err(keyring::Error::NoEntry) => {
            let mut key_bytes = [0u8; 32];
            rand::rng().fill_bytes(&mut key_bytes);
            let new_key = hex::encode(key_bytes);
            entry.set_password(&new_key).map_err(|e| e.to_string())?;
            Ok(new_key)
        },
        Err(e) => Err(e.to_string()),
    }
}

fn get_master_key_bytes() -> Result<[u8; 32], String> {
    let hex_key = get_or_create_master_key()?;
    let bytes = hex::decode(hex_key).map_err(|e| e.to_string())?;
    if bytes.len() != 32 {
        return Err("Invalid master key length".into());
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&bytes);
    Ok(out)
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Api {
    pub key: String,
}

impl Api {
    pub fn encrypt(&self) -> Result<Vec<u8>, String> {
        let master = get_master_key_bytes()?;
        let cipher = Aes256Gcm::new_from_slice(&master).map_err(|e| e.to_string())?;
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let plaintext = serde_json::to_vec(self).map_err(|e| e.to_string())?;
        let ciphertext = cipher.encrypt(&nonce, plaintext.as_ref()).map_err(|e| e.to_string())?;

        let mut out = nonce.to_vec();
        out.extend_from_slice(&ciphertext);
        Ok(out)
    }

    pub fn decrypt(blob: &[u8]) -> Result<Self, String> {
        if blob.len() < 12 {
            return Err("Invalid encrypted API format".into());
        }
        let master = get_master_key_bytes()?;
        let cipher = Aes256Gcm::new_from_slice(&master).map_err(|e| e.to_string())?;
        let (nonce_bytes, ciphertext) = blob.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);
        let plaintext = cipher.decrypt(nonce, ciphertext).map_err(|_| "Failed to decrypt API".to_string())?;
        let api: Api = serde_json::from_slice(&plaintext).map_err(|e| e.to_string())?;
        Ok(api)
    }
}
