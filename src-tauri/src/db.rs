use rusqlite::{Connection, Result, params};
use std::path::Path;

/// Open (or create) the database at the given path.
/// Called ONCE at startup; the connection is stored in `AppState`.
pub fn init_db(app_dir: &Path) -> Result<Connection> {
    std::fs::create_dir_all(app_dir).ok();
    let db_path = app_dir.join("history.db");
    let conn = Connection::open(db_path)?;

    // Enable WAL for better concurrent read perf
    conn.execute_batch("PRAGMA journal_mode=WAL;")?;

    // RAG schema migration — safe to run every startup; ignored if columns already exist
    let _ = conn.execute("ALTER TABLE history ADD COLUMN embedding     BLOB", []);
    let _ = conn.execute("ALTER TABLE history ADD COLUMN content_hash  TEXT", []);
    let _ = conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_history_mode_ts
             ON history(mode, timestamp DESC)", [],
    );
    let _ = conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_history_no_embedding
             ON history(id) WHERE embedding IS NULL", [],
    );

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS history (
            id              INTEGER PRIMARY KEY,
            timestamp       DATETIME DEFAULT CURRENT_TIMESTAMP,
            input_preview   TEXT,
            mode            TEXT,
            output          TEXT,
            char_count      INTEGER,
            tone_score      INTEGER DEFAULT 0,
            formality_score INTEGER DEFAULT 5,
            embedding       BLOB,
            content_hash    TEXT
        );

        CREATE TABLE IF NOT EXISTS intent_corrections (
            id               INTEGER PRIMARY KEY,
            timestamp        DATETIME DEFAULT CURRENT_TIMESTAMP,
            suggested_intent TEXT NOT NULL,
            chosen_intent    TEXT NOT NULL,
            confidence       REAL NOT NULL,
            text_length      INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS voice_profile (
            id           INTEGER PRIMARY KEY,
            feature_type TEXT NOT NULL,
            feature_key  TEXT NOT NULL,
            value        TEXT,
            count        INTEGER DEFAULT 1,
            last_seen    DATETIME DEFAULT CURRENT_TIMESTAMP,
            UNIQUE(feature_type, feature_key)
        );

        CREATE TABLE IF NOT EXISTS context_memory (
            id           INTEGER PRIMARY KEY,
            entity_type  TEXT NOT NULL,
            entity_name  TEXT NOT NULL,
            attribute    TEXT NOT NULL,
            value        TEXT,
            last_seen    DATETIME DEFAULT CURRENT_TIMESTAMP,
            seen_count   INTEGER DEFAULT 1,
            UNIQUE(entity_type, entity_name, attribute)
        );
        CREATE TABLE IF NOT EXISTS config (
            key_name TEXT PRIMARY KEY,
            value    TEXT
        );

        CREATE TABLE IF NOT EXISTS audit_log (
            id         INTEGER PRIMARY KEY,
            timestamp  DATETIME DEFAULT CURRENT_TIMESTAMP,
            mode       TEXT NOT NULL,
            ai_mode    TEXT NOT NULL,
            char_count INTEGER NOT NULL,
            was_stored INTEGER NOT NULL DEFAULT 1
        );

        CREATE TABLE IF NOT EXISTS reply_feedback (
            id           INTEGER PRIMARY KEY,
            timestamp    DATETIME DEFAULT CURRENT_TIMESTAMP,
            input_preview TEXT NOT NULL,
            ai_output    TEXT NOT NULL,
            accepted     INTEGER NOT NULL DEFAULT 0,
            contact_hint TEXT
        );",
    )?;

    let _ = conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_feedback_contact
             ON reply_feedback(contact_hint, accepted, timestamp DESC)",
        [],
    );

    Ok(conn)
}

/// Delete history entries older than `days`. Called at startup.
pub fn cleanup_old_history(conn: &Connection, days: i64) {
    let _ = conn.execute(
        "DELETE FROM history WHERE timestamp < datetime('now', printf('-%d days', ?1))",
        params![days],
    );
}

pub fn set_config(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO config (key_name, value) VALUES (?1, ?2)
         ON CONFLICT(key_name) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

pub fn get_config(conn: &Connection, key: &str) -> Result<String> {
    conn.query_row(
        "SELECT value FROM config WHERE key_name = ?1",
        params![key],
        |row| row.get(0),
    )
}

/// Append a row to history and prune to the most recent 500.
/// `content_hash` — SHA-256 hex of (preview + mode) for deduplication.
pub fn save_history(
    conn: &Connection,
    preview: &str,
    mode: &str,
    output: &str,
    tone_score: i32,
    formality_score: i32,
    embedding:     Option<&[u8]>,
    content_hash:  Option<&str>,
) -> Result<()> {
    // Dedup: skip if identical content+mode already stored
    if let Some(hash) = content_hash {
        let exists: bool = conn.query_row(
            "SELECT 1 FROM history WHERE content_hash = ?1 AND mode = ?2 LIMIT 1",
            params![hash, mode],
            |_| Ok(true),
        ).unwrap_or(false);
        if exists { return Ok(()); }
    }

    conn.execute(
        "INSERT INTO history
             (input_preview, mode, output, char_count, tone_score, formality_score, embedding, content_hash)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            preview, mode, output, output.len() as i32,
            tone_score as i32, formality_score as i32,
            embedding, content_hash
        ],
    )?;
    // Prune to 500 most recent (was 100 — more history = better RAG)
    conn.execute(
        "DELETE FROM history WHERE id NOT IN (
            SELECT id FROM history ORDER BY timestamp DESC LIMIT 500
         )",
        [],
    )?;
    Ok(())
}

/// Store an embedding BLOB for an existing history row (called from background task).
pub fn update_embedding(conn: &Connection, id: i64, embedding: &[u8]) -> Result<()> {
    conn.execute("UPDATE history SET embedding = ?1 WHERE id = ?2", params![embedding, id])?;
    Ok(())
}

/// Look up the most recently inserted history row for a given (preview, mode) pair.
/// Used by the background embedding task to find the row to update.
pub fn get_last_history_id(conn: &Connection, preview: &str, mode: &str) -> Option<i64> {
    conn.query_row(
        "SELECT id FROM history WHERE input_preview = ?1 AND mode = ?2 ORDER BY timestamp DESC LIMIT 1",
        params![preview, mode],
        |row| row.get(0),
    ).ok()
}

/// Fetch history rows including their embeddings for semantic RAG scoring.
/// Returns (id, input_preview, output, embedding_bytes_or_none).
pub fn get_history_with_embeddings(
    conn: &Connection,
    mode: &str,
    limit: i64,
) -> Result<Vec<(i64, String, String, Option<Vec<u8>>)>> {
    let mut stmt = conn.prepare(
        "SELECT id, input_preview, output, embedding
           FROM history
          WHERE mode = ?1 AND input_preview != '' AND output != ''
          ORDER BY timestamp DESC LIMIT ?2",
    )?;
    let rows = stmt.query_map(params![mode, limit], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, Option<Vec<u8>>>(3)?,
        ))
    })?;
    rows.collect()
}


/// Load the top (suggested → chosen) correction pairs that have happened ≥ 3 times.
/// Used at startup to seed the adaptive mode-suggestion override map.
pub fn get_top_corrections(conn: &Connection) -> Result<Vec<(String, String)>> {
    let mut stmt = conn.prepare(
        "SELECT suggested_intent, chosen_intent, COUNT(*) as cnt
           FROM intent_corrections
          GROUP BY suggested_intent, chosen_intent
         HAVING cnt >= 3
          ORDER BY cnt DESC
          LIMIT 20"
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    rows.collect()
}

/// Record that the user chose a different intent than suggested.
pub fn save_correction(
    conn: &Connection,
    suggested: &str,
    chosen: &str,
    confidence: f32,
    text_length: usize,
) -> Result<()> {
    conn.execute(
        "INSERT INTO intent_corrections (suggested_intent, chosen_intent, confidence, text_length)
         VALUES (?1, ?2, ?3, ?4)",
        params![suggested, chosen, confidence, text_length as i64],
    )?;
    Ok(())
}


/// --- Feature 1: Personal Voice Engine (Moat) ---

pub fn observe_session(
    conn: &rusqlite::Connection,
    text: &str,
    tone: i32,
    formality: i32,
    _word_count: usize,
) -> rusqlite::Result<()> {
    observe_session_v2(conn, text, tone, formality, 0.0, 0.0, 0)
}

pub fn observe_session_v2(
    conn: &rusqlite::Connection,
    text: &str,
    tone: i32,
    formality: i32,
    contraction_rate: f32,
    avg_sentence_len: f32,
    emoji_count: usize,
) -> rusqlite::Result<()> {
    let words: Vec<&str> = text.split_whitespace().collect();

    // 1. Opener (first alphabetic word)
    if let Some(first) = words.first() {
        let clean = first.trim_matches(|c: char| !c.is_alphabetic());
        if !clean.is_empty() && clean.len() < 15 {
            conn.execute(
                "INSERT INTO voice_profile (feature_type, feature_key, count) \
                 VALUES ('opener', ?, 1) \
                 ON CONFLICT(feature_type, feature_key) DO UPDATE SET count = count + 1, last_seen = CURRENT_TIMESTAMP",
                [clean.to_lowercase()],
            )?;
        }
    }

    // 2. Closer (last alphabetic word)
    if let Some(last) = words.last() {
        let clean = last.trim_matches(|c: char| !c.is_alphabetic());
        if !clean.is_empty() && clean.len() < 15 {
            conn.execute(
                "INSERT INTO voice_profile (feature_type, feature_key, count) \
                 VALUES ('closer', ?, 1) \
                 ON CONFLICT(feature_type, feature_key) DO UPDATE SET count = count + 1, last_seen = CURRENT_TIMESTAMP",
                [clean.to_lowercase()],
            )?;
        }
    }

    // 3. Formality & Tone rolling averages
    for (key, val) in &[("formality", formality as f64), ("tone", tone as f64)] {
        conn.execute(
            "INSERT INTO voice_profile (feature_type, feature_key, value, count) \
             VALUES ('stat', ?, ?, 1) \
             ON CONFLICT(feature_type, feature_key) DO UPDATE SET \
               value = ((CAST(value AS REAL) * count) + ?) / (count + 1), count = count + 1",
            rusqlite::params![key, val.to_string(), val],
        )?;
    }

    // 4. Contraction rate rolling average
    conn.execute(
        "INSERT INTO voice_profile (feature_type, feature_key, value, count) \
         VALUES ('stat', 'contraction_rate', ?, 1) \
         ON CONFLICT(feature_type, feature_key) DO UPDATE SET \
           value = ((CAST(value AS REAL) * count) + ?) / (count + 1), count = count + 1",
        [contraction_rate.to_string(), contraction_rate.to_string()],
    )?;

    // 5. Average sentence length rolling average
    conn.execute(
        "INSERT INTO voice_profile (feature_type, feature_key, value, count) \
         VALUES ('stat', 'avg_sentence_len', ?, 1) \
         ON CONFLICT(feature_type, feature_key) DO UPDATE SET \
           value = ((CAST(value AS REAL) * count) + ?) / (count + 1), count = count + 1",
        [avg_sentence_len.to_string(), avg_sentence_len.to_string()],
    )?;

    // 6. Emoji usage flag (ratio of sessions that used emoji)
    let emoji_used = if emoji_count > 0 { 1.0_f64 } else { 0.0_f64 };
    conn.execute(
        "INSERT INTO voice_profile (feature_type, feature_key, value, count) \
         VALUES ('stat', 'emoji_rate', ?, 1) \
         ON CONFLICT(feature_type, feature_key) DO UPDATE SET \
           value = ((CAST(value AS REAL) * count) + ?) / (count + 1), count = count + 1",
        [emoji_used.to_string(), emoji_used.to_string()],
    )?;

    Ok(())
}

/// Record a per-contact communication pattern (opener/closer used with a specific person).
pub fn record_contact_pattern(
    conn: &Connection,
    entity_name: &str,
    attr: &str,    // "opener" or "closer"
    value: &str,
) -> Result<()> {
    conn.execute(
        "INSERT INTO context_memory (entity_type, entity_name, attribute, value) \
         VALUES ('person', ?1, ?2, ?3) \
         ON CONFLICT(entity_type, entity_name, attribute) DO UPDATE SET \
           value = CASE WHEN seen_count < 3 THEN excluded.value ELSE value END, \
           seen_count = seen_count + 1, \
           last_seen = CURRENT_TIMESTAMP",
        params![entity_name, attr, value],
    )?;
    Ok(())
}

pub fn get_voice_profile(conn: &rusqlite::Connection) -> rusqlite::Result<Vec<(String, String, String)>> {
    let mut stmt = conn.prepare(
        "SELECT feature_type, feature_key, IFNULL(value, '') FROM voice_profile \
         WHERE count >= 1 \
         ORDER BY feature_type, count DESC LIMIT 50"
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get(0)?, row.get(1)?, row.get(2)?))
    })?;

    let mut res = Vec::new();
    for row in rows {
        res.push(row?);
    }
    Ok(res)
}

/// --- Feature 2: Context Memory (Moat) ---

pub fn record_entity_mention(
    conn: &Connection,
    entity_type: &str,
    entity_name: &str,
    tone: i32,
    formality: i32,
) -> Result<()> {
    // 1. Update/Insert Typical Tone
    conn.execute(
        "INSERT INTO context_memory (entity_type, entity_name, attribute, value) \
         VALUES (?1, ?2, 'typical_tone', ?3) \
         ON CONFLICT(entity_type, entity_name, attribute) DO UPDATE SET \
           value = (CAST(value AS REAL) * seen_count + ?3) / (seen_count + 1), \
           seen_count = seen_count + 1, \
           last_seen = CURRENT_TIMESTAMP",
        params![entity_type, entity_name, tone as f64],
    )?;

    // 2. Update/Insert Relationship/Formality
    conn.execute(
        "INSERT INTO context_memory (entity_type, entity_name, attribute, value) \
         VALUES (?1, ?2, 'formality', ?3) \
         ON CONFLICT(entity_type, entity_name, attribute) DO UPDATE SET \
           value = (CAST(value AS REAL) * seen_count + ?3) / (seen_count + 1), \
           last_seen = CURRENT_TIMESTAMP",
        params![entity_type, entity_name, formality as f64],
    )?;

    Ok(())
}

pub fn get_entities_context(conn: &Connection, names: &[String]) -> Result<Vec<(String, String, String, String)>> {
    if names.is_empty() { return Ok(vec![]); }
    
    // Simple IN clause build
    let placeholders: String = names.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let query = format!(
        "SELECT entity_name, entity_type, attribute, value FROM context_memory \
         WHERE entity_name IN ({}) AND seen_count >= 1", 
        placeholders
    );

    let mut stmt = conn.prepare(&query)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(names), |row| {
        Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
    })?;

    let mut res = Vec::new();
    for row in rows {
        res.push(row?);
    }
    Ok(res)
}

/// --- Feature 4: Communication Score (Moat) ---

#[derive(Debug, serde::Serialize)]
pub struct CommReport {
    pub avg_tone: f32,
    pub avg_formality: f32,
    pub total_sessions: i64,
    pub frequent_entities: Vec<String>,
    pub friction_hotspots: Vec<String>,
}

pub fn get_communication_report(conn: &Connection) -> Result<CommReport> {
    let stats: (f32, f32, i64) = conn.query_row(
        "SELECT AVG(tone_score), AVG(formality_score), COUNT(*) FROM history \
         WHERE timestamp > datetime('now', '-7 days')",
        [],
        |r| Ok((r.get::<_, Option<f32>>(0)?.unwrap_or(0.0), r.get::<_, Option<f32>>(1)?.unwrap_or(5.0), r.get(2)?)),
    )?;

    let mut stmt = conn.prepare(
        "SELECT entity_name FROM context_memory \
         WHERE last_seen > datetime('now', '-7 days') \
         ORDER BY seen_count DESC LIMIT 5"
    )?;
    let entities = stmt.query_map([], |r| r.get(0))?
        .collect::<Result<Vec<String>>>()?;

    let mut stmt = conn.prepare(
        "SELECT entity_name FROM context_memory \
         WHERE attribute = 'typical_tone' AND CAST(value AS REAL) < -1.0 \
         ORDER BY last_seen DESC LIMIT 3"
    )?;
    let friction = stmt.query_map([], |r| r.get(0))?
        .collect::<Result<Vec<String>>>()?;

    Ok(CommReport {
        avg_tone: stats.0,
        avg_formality: stats.1,
        total_sessions: stats.2,
        frequent_entities: entities,
        friction_hotspots: friction,
    })
}

/// Store detected language for a contact so future replies auto-match it.
pub fn record_contact_language(
    conn: &Connection,
    entity_name: &str,
    language: &str,
) -> Result<()> {
    conn.execute(
        "INSERT INTO context_memory (entity_type, entity_name, attribute, value)
         VALUES ('person', ?1, 'language', ?2)
         ON CONFLICT(entity_type, entity_name, attribute) DO UPDATE SET
           value = excluded.value,
           last_seen = CURRENT_TIMESTAMP,
           seen_count = seen_count + 1",
        params![entity_name, language],
    )?;
    Ok(())
}

/// Retrieve the stored language for a contact (only if seen >= 2 times).
pub fn get_contact_language(
    conn: &Connection,
    entity_name: &str,
) -> Result<Option<String>> {
    use rusqlite::OptionalExtension;
    conn.query_row(
        "SELECT value FROM context_memory
         WHERE entity_name = ?1 AND attribute = 'language' AND seen_count >= 2",
        params![entity_name],
        |row| row.get(0),
    ).optional()
}

// ── History read-back ──────────────────────────────────────────────────────

#[derive(Debug, serde::Serialize)]
pub struct HistoryEntry {
    pub id: i64,
    pub timestamp: String,
    pub input_preview: String,
    pub mode: String,
    pub output: String,
}

// ── Audit log ──────────────────────────────────────────────────────────────

pub fn save_audit_entry(
    conn: &Connection,
    mode: &str,
    ai_mode: &str,
    char_count: usize,
    was_stored: bool,
) -> Result<()> {
    conn.execute(
        "INSERT INTO audit_log (mode, ai_mode, char_count, was_stored) VALUES (?1, ?2, ?3, ?4)",
        params![mode, ai_mode, char_count as i64, was_stored as i64],
    )?;
    // Keep last 500 audit entries
    conn.execute(
        "DELETE FROM audit_log WHERE id NOT IN (SELECT id FROM audit_log ORDER BY timestamp DESC LIMIT 500)",
        [],
    )?;
    Ok(())
}

#[derive(Debug, serde::Serialize)]
pub struct AuditEntry {
    pub id: i64,
    pub timestamp: String,
    pub mode: String,
    pub ai_mode: String,
    pub char_count: i64,
    pub was_stored: bool,
}

pub fn get_audit_log(conn: &Connection, limit: i64) -> Result<Vec<AuditEntry>> {
    let mut stmt = conn.prepare(
        "SELECT id, timestamp, mode, ai_mode, char_count, was_stored
         FROM audit_log ORDER BY timestamp DESC LIMIT ?1"
    )?;
    let rows = stmt.query_map(params![limit], |row| {
        Ok(AuditEntry {
            id:         row.get(0)?,
            timestamp:  row.get(1)?,
            mode:       row.get(2)?,
            ai_mode:    row.get(3)?,
            char_count: row.get(4)?,
            was_stored: row.get::<_, i64>(5)? != 0,
        })
    })?;
    rows.collect()
}

// ── History read-back ──────────────────────────────────────────────────────

pub fn get_recent_history(conn: &Connection, limit: i64) -> Result<Vec<HistoryEntry>> {
    let mut stmt = conn.prepare(
        "SELECT id, timestamp, IFNULL(input_preview,''), IFNULL(mode,''), IFNULL(output,'')
         FROM history ORDER BY timestamp DESC LIMIT ?1"
    )?;
    let rows = stmt.query_map(params![limit], |row| {
        Ok(HistoryEntry {
            id:            row.get(0)?,
            timestamp:     row.get(1)?,
            input_preview: row.get(2)?,
            mode:          row.get(3)?,
            output:        row.get(4)?,
        })
    })?;
    rows.collect()
}

// ── Reply feedback ─────────────────────────────────────────────────────────

/// Save whether the user accepted or rejected an AI-generated reply.
/// `contact_hint` — the contact name detected from the message (may be None).
pub fn save_reply_feedback(
    conn: &Connection,
    input_preview: &str,
    ai_output: &str,
    accepted: bool,
    contact_hint: Option<&str>,
) -> Result<()> {
    conn.execute(
        "INSERT INTO reply_feedback (input_preview, ai_output, accepted, contact_hint)
         VALUES (?1, ?2, ?3, ?4)",
        params![input_preview, ai_output, accepted as i64, contact_hint],
    )?;
    // Keep last 1000 feedback rows
    conn.execute(
        "DELETE FROM reply_feedback WHERE id NOT IN (
             SELECT id FROM reply_feedback ORDER BY timestamp DESC LIMIT 1000
         )",
        [],
    )?;
    Ok(())
}

/// Return the most recent accepted (input, reply) pairs for a specific contact.
/// Used as few-shot RAG examples in the Reply prompt.
pub fn get_accepted_reply_examples(
    conn: &Connection,
    contact_hint: &str,
    limit: i64,
) -> Result<Vec<(String, String)>> {
    let mut stmt = conn.prepare(
        "SELECT input_preview, ai_output FROM reply_feedback
          WHERE accepted = 1 AND contact_hint = ?1
          ORDER BY timestamp DESC LIMIT ?2",
    )?;
    let rows = stmt.query_map(params![contact_hint, limit], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    rows.collect()
}