use crate::db;
use crate::models::{ClipboardItem, Settings};
use crate::paths::sync_file_path;
use crate::settings_store;
use base64::Engine as _;
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};
use zeroize::Zeroize;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SyncPlain {
    v: u32,
    updated_at_ms: i64,
    device_id: String,
    items: Vec<ClipboardItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SyncEncryptedFile {
    v: u32,
    salt_b64: String,
    nonce_b64: String,
    ct_b64: String,
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn derive_key(passphrase: &str, salt: &[u8]) -> Result<[u8; 32], String> {
    use argon2::Argon2;

    let argon2 = Argon2::default();
    let mut out = [0u8; 32];
    argon2
        .hash_password_into(passphrase.as_bytes(), salt, &mut out)
        .map_err(|e| format!("argon2 failed: {e}"))?;
    Ok(out)
}

fn encrypt(passphrase: &str, salt: &[u8], plaintext: &[u8]) -> Result<SyncEncryptedFile, String> {
    let mut key_bytes = derive_key(passphrase, salt)?;
    let key = Key::from_slice(&key_bytes);
    let cipher = ChaCha20Poly1305::new(key);

    let mut nonce_bytes = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ct = cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| format!("encrypt failed: {e}"))?;

    key_bytes.zeroize();

    Ok(SyncEncryptedFile {
        v: 1,
        salt_b64: base64::engine::general_purpose::STANDARD.encode(salt),
        nonce_b64: base64::engine::general_purpose::STANDARD.encode(nonce_bytes),
        ct_b64: base64::engine::general_purpose::STANDARD.encode(ct),
    })
}

fn decrypt(passphrase: &str, enc: &SyncEncryptedFile) -> Result<Vec<u8>, String> {
    let salt = base64::engine::general_purpose::STANDARD
        .decode(&enc.salt_b64)
        .map_err(|e| format!("invalid salt_b64: {e}"))?;
    let nonce = base64::engine::general_purpose::STANDARD
        .decode(&enc.nonce_b64)
        .map_err(|e| format!("invalid nonce_b64: {e}"))?;
    let ct = base64::engine::general_purpose::STANDARD
        .decode(&enc.ct_b64)
        .map_err(|e| format!("invalid ct_b64: {e}"))?;

    if nonce.len() != 12 {
        return Err("invalid nonce length".to_string());
    }

    let mut key_bytes = derive_key(passphrase, &salt)?;
    let key = Key::from_slice(&key_bytes);
    let cipher = ChaCha20Poly1305::new(key);

    let pt = cipher
        .decrypt(Nonce::from_slice(&nonce), ct.as_ref())
        .map_err(|_| "decrypt failed (wrong passphrase?)".to_string());

    key_bytes.zeroize();
    pt
}

fn salt_from_settings(settings: &Settings) -> Result<Vec<u8>, String> {
    let b64 = settings
        .sync_salt_b64
        .as_ref()
        .ok_or_else(|| "sync salt missing".to_string())?;
    base64::engine::general_purpose::STANDARD
        .decode(b64)
        .map_err(|e| format!("invalid sync_salt_b64: {e}"))
}

pub fn export_now(app: &tauri::AppHandle) -> Result<(), String> {
    let mut settings = settings_store::load_or_init_settings(app)?;
    if !settings.sync_enabled {
        return Ok(());
    }
    let folder = match settings.sync_folder.clone() {
        Some(f) if !f.trim().is_empty() => f,
        _ => return Err("sync is enabled but sync folder is not set".to_string()),
    };

    settings = settings_store::ensure_sync_salt_b64(app, settings)?;
    let salt = salt_from_settings(&settings)?;

    let passphrase = settings_store::load_sync_passphrase()?
        .ok_or_else(|| "sync passphrase not set".to_string())?;

    let items = db::list_items(app, 5000, None)?;

    let plain = SyncPlain {
        v: 1,
        updated_at_ms: now_ms(),
        device_id: settings.device_id.clone(),
        items,
    };

    let plain_bytes = serde_json::to_vec(&plain)
        .map_err(|e| format!("failed to serialize sync payload: {e}"))?;

    let enc = encrypt(&passphrase, &salt, &plain_bytes)?;
    let file_path = sync_file_path(app, &folder)?;

    let tmp_path = file_path.with_extension("tmp");
    let raw = serde_json::to_vec_pretty(&enc)
        .map_err(|e| format!("failed to serialize encrypted file: {e}"))?;
    fs::write(&tmp_path, raw).map_err(|e| format!("failed to write tmp sync file: {e}"))?;
    fs::rename(&tmp_path, &file_path).map_err(|e| format!("failed to move sync file into place: {e}"))?;

    Ok(())
}

pub fn import_now(app: &tauri::AppHandle) -> Result<u32, String> {
    let settings = settings_store::load_or_init_settings(app)?;
    if !settings.sync_enabled {
        return Ok(0);
    }
    let folder = match settings.sync_folder.clone() {
        Some(f) if !f.trim().is_empty() => f,
        _ => return Err("sync is enabled but sync folder is not set".to_string()),
    };

    let file_path = sync_file_path(app, &folder)?;
    let raw = match fs::read_to_string(&file_path) {
        Ok(v) => v,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(e) => return Err(format!("failed to read sync file: {e}")),
    };

    let enc: SyncEncryptedFile = serde_json::from_str(&raw)
        .map_err(|e| format!("failed to parse sync file: {e}"))?;

    let passphrase = settings_store::load_sync_passphrase()?
        .ok_or_else(|| "sync passphrase not set".to_string())?;

    let pt = decrypt(&passphrase, &enc)?;
    let plain: SyncPlain = serde_json::from_slice(&pt)
        .map_err(|e| format!("failed to parse decrypted payload: {e}"))?;

    let inserted = db::upsert_items(app, &plain.items)?;
    Ok(inserted)
}
