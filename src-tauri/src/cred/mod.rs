use tauri::{AppHandle, Manager};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use keyring::Entry;
use rand::RngCore;
use aes_gcm::{
    AeadCore, Aes256Gcm, Nonce, aead::{Aead, KeyInit, OsRng}
};

fn get_storage_path(app: &AppHandle) -> Result<PathBuf, String> {
    let path = app.path().app_local_data_dir()
        .map_err(|e| e.to_string())?
        .join("keys.dat");
    Ok(path)
}

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

#[tauri::command]
pub(crate) fn save_api_key(app: AppHandle, provider: String, key: String) -> Result<(), String> {
    let master_key_hex = get_or_create_master_key()?;
    let master_key_bytes = hex::decode(master_key_hex).map_err(|e| e.to_string())?;
    let storage_path = get_storage_path(&app)?;
    
    let mut all_keys: HashMap<String, String> = match fs::read_to_string(&storage_path) {
        Ok(b64_content) => {
            if b64_content.is_empty() {
                HashMap::new()
            } else {
                let encrypted_bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, b64_content)
                    .map_err(|_| "Failed to decode base64 content".to_string())?;

                let cipher = Aes256Gcm::new_from_slice(&master_key_bytes).map_err(|e| e.to_string())?;
                let (nonce_bytes, ciphertext) = encrypted_bytes.split_at(12);
                let nonce = Nonce::from_slice(nonce_bytes);
                
                let decrypted_json_bytes = cipher.decrypt(nonce, ciphertext)
                    .map_err(|_| "Invalid master key or corrupt data".to_string())?;
                
                serde_json::from_slice(&decrypted_json_bytes).unwrap_or_default()
            }
        },
        Err(_) => HashMap::new(),
    };

    all_keys.insert(provider, key);

    let json_bytes = serde_json::to_string(&all_keys).map_err(|e| e.to_string())?.into_bytes();
    
    let cipher = Aes256Gcm::new_from_slice(&master_key_bytes).map_err(|e| e.to_string())?;
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng); 
    let ciphertext = cipher.encrypt(&nonce, json_bytes.as_ref())
        .map_err(|e| e.to_string())?;

    let mut encrypted_bytes_with_nonce = nonce.to_vec();
    encrypted_bytes_with_nonce.extend_from_slice(&ciphertext);
    
    let b64_content = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, encrypted_bytes_with_nonce);
    
    if let Some(parent_dir) = storage_path.parent() {
        fs::create_dir_all(parent_dir).map_err(|e| e.to_string())?;
    }

    fs::write(storage_path, b64_content).map_err(|e| e.to_string())?;

    Ok(())
}


#[tauri::command]
pub(crate) fn get_api_key(app: AppHandle, provider: String) -> Result<String, String> {
    let master_key_hex = get_or_create_master_key()?;
    let master_key_bytes = hex::decode(master_key_hex).map_err(|e| e.to_string())?;
    let storage_path = get_storage_path(&app)?;

    let b64_content = fs::read_to_string(storage_path)
        .map_err(|_| "Key file not found.".to_string())?;
        
    let encrypted_bytes_with_nonce = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, b64_content)
        .map_err(|_| "Failed to decode base64 content".to_string())?;

    let cipher = Aes256Gcm::new_from_slice(&master_key_bytes).map_err(|e| e.to_string())?;
    

    if encrypted_bytes_with_nonce.len() < 12 {
        return Err("Invalid encrypted data format".to_string());
    }
    let (nonce_bytes, ciphertext) = encrypted_bytes_with_nonce.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);

    let decrypted_json_bytes = cipher.decrypt(nonce, ciphertext)
        .map_err(|_| "Failed to decrypt data.".to_string())?;
        
    let all_keys: HashMap<String, String> = serde_json::from_slice(&decrypted_json_bytes)
        .map_err(|_| "Failed to parse key data.".to_string())?;

    all_keys.get(&provider)
        .cloned()
        .ok_or_else(|| format!("Key for provider '{}' not found.", provider))
}