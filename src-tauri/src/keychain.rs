use keyring::Entry;
use zeroize::Zeroizing;
use crate::db;

const SERVICE: &str = "snaptext";
const KEY_NAME: &str = "byok_api_key";

// ── Public API ─────────────────────────────────────────────────────────────

pub fn store_api_key(conn: &rusqlite::Connection, key: &str) -> Result<(), String> {
    let entry = Entry::new(SERVICE, KEY_NAME).map_err(|e| e.to_string())?;
    entry.set_password(key).map_err(|e| e.to_string())?;
    // Clear old XOR value from DB if present
    let _ = db::set_config(conn, "gemini_key", "");
    Ok(())
}

pub fn get_api_key(conn: &rusqlite::Connection) -> Result<String, String> {
    // Try Credential Manager first
    if let Ok(entry) = Entry::new(SERVICE, KEY_NAME) {
        if let Ok(key) = entry.get_password() {
            if !key.trim().is_empty() {
                return Ok(key);
            }
        }
    }
    // Migrate legacy XOR key from DB on first run after upgrade
    migrate_legacy_key(conn)
}

pub fn delete_api_key(conn: &rusqlite::Connection) -> Result<(), String> {
    if let Ok(entry) = Entry::new(SERVICE, KEY_NAME) {
        let _ = entry.delete_password();
    }
    db::set_config(conn, "gemini_key", "").map_err(|e| e.to_string())
}

// ── Legacy migration ───────────────────────────────────────────────────────

fn migrate_legacy_key(conn: &rusqlite::Connection) -> Result<String, String> {
    let hex_val = db::get_config(conn, "gemini_key").map_err(|_| "No key found".to_string())?;
    if hex_val.is_empty() {
        return Err("No key found".into());
    }

    // Decrypt legacy XOR key
    let mut encrypted: Vec<u8> = (0..hex_val.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex_val[i..i + 2], 16).unwrap_or(0))
        .collect();

    let machine = std::env::var("COMPUTERNAME").unwrap_or_else(|_| "ANTIGRAVITY_SALT".to_string());
    let key_bytes = machine.as_bytes();
    for (i, b) in encrypted.iter_mut().enumerate() {
        *b ^= key_bytes[i % key_bytes.len()];
    }

    let decrypted = Zeroizing::new(
        String::from_utf8(encrypted).map_err(|e| e.to_string())?
    );

    if decrypted.trim().is_empty() {
        return Err("No key found".into());
    }

    // Migrate: store in Credential Manager, wipe from DB
    if let Ok(entry) = Entry::new(SERVICE, KEY_NAME) {
        if entry.set_password(&decrypted).is_ok() {
            let _ = db::set_config(conn, "gemini_key", "");
        }
    }

    Ok(decrypted.to_string())
}
