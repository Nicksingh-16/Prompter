use crate::db;

fn crypt(data: &str) -> Vec<u8> {
    let key = std::env::var("COMPUTERNAME").unwrap_or_else(|_| "ANTIGRAVITY_SALT".to_string());
    data.as_bytes().iter().enumerate().map(|(i, &b)| {
        b ^ key.as_bytes()[i % key.len()]
    }).collect()
}

pub fn store_api_key(conn: &rusqlite::Connection, key: &str) -> Result<(), String> {
    println!(">>> KEYCHAIN: storing obfuscated key in DB");
    let encrypted = crypt(key);
    // Use simple hex encoding to store in TEXT field
    let hex_val: String = encrypted.iter().map(|b| format!("{:02x}", b)).collect();
    db::set_config(conn, "gemini_key", &hex_val).map_err(|e| e.to_string())
}

pub fn get_api_key(conn: &rusqlite::Connection) -> Result<String, String> {
    let hex_val = db::get_config(conn, "gemini_key").map_err(|_| "No key found".to_string())?;
    if hex_val.is_empty() { return Err("No key found".into()); }
    
    let encrypted = (0..hex_val.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex_val[i..i + 2], 16).unwrap_or(0))
        .collect::<Vec<u8>>();
        
    let decrypted = String::from_utf8(crypt(&String::from_utf8_lossy(&encrypted))).map_err(|e| e.to_string())?;
    Ok(decrypted)
}

pub fn delete_api_key(conn: &rusqlite::Connection) -> Result<(), String> {
    db::set_config(conn, "gemini_key", "").map_err(|e| e.to_string())
}
