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

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS history (
            id              INTEGER PRIMARY KEY,
            timestamp       DATETIME DEFAULT CURRENT_TIMESTAMP,
            input_preview   TEXT,
            mode            TEXT,
            output          TEXT,
            char_count      INTEGER,
            tone_score      INTEGER DEFAULT 0,
            formality_score INTEGER DEFAULT 5
        );

        CREATE TABLE IF NOT EXISTS intent_corrections (
            id               INTEGER PRIMARY KEY,
            timestamp        DATETIME DEFAULT CURRENT_TIMESTAMP,
            suggested_intent TEXT NOT NULL,
            chosen_intent    TEXT NOT NULL,
            confidence       REAL NOT NULL,
            text_length      INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS intent_weight_overrides (
            id           INTEGER PRIMARY KEY,
            from_intent  TEXT NOT NULL,
            to_intent    TEXT NOT NULL,
            adjustment   REAL NOT NULL DEFAULT 0.0,
            sample_count INTEGER NOT NULL DEFAULT 1,
            UNIQUE(from_intent, to_intent)
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
        );",
    )?;

    Ok(conn)
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

/// Append a row to history and prune to the most recent 100.
pub fn save_history(
    conn: &Connection,
    preview: &str,
    mode: &str,
    output: &str,
    tone_score: i32,
    formality_score: i32,
) -> Result<()> {
    conn.execute(
        "INSERT INTO history (input_preview, mode, output, char_count, tone_score, formality_score)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![preview, mode, output, output.len() as i32, tone_score as i32, formality_score as i32],
    )?;
    // Prune to 100 most recent
    conn.execute(
        "DELETE FROM history WHERE id NOT IN (
            SELECT id FROM history ORDER BY timestamp DESC LIMIT 100
         )",
        [],
    )?;
    Ok(())
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
    // Recompute override for this pair after every 3 corrections
    recompute_weight_override(conn, suggested, chosen)?;
    Ok(())
}

/// Recompute the weight adjustment for a (from→to) pair.
/// Returns the new adjustment value.
fn recompute_weight_override(conn: &Connection, from: &str, to: &str) -> Result<()> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM intent_corrections WHERE suggested_intent=?1 AND chosen_intent=?2",
        params![from, to],
        |r| r.get(0),
    )?;

    if count < 3 {
        return Ok(()); // not enough data yet
    }

    // Adjustment grows logarithmically: 0.1 * log2(count)
    let adjustment = 0.1 * (count as f64).log2() as f32;

    conn.execute(
        "INSERT INTO intent_weight_overrides (from_intent, to_intent, adjustment, sample_count)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(from_intent, to_intent) DO UPDATE
           SET adjustment=excluded.adjustment,
               sample_count=excluded.sample_count",
        params![from, to, adjustment, count],
    )?;
    Ok(())
}

/// Load all learned weight overrides. Used in lib.rs before intent scoring.


/// --- Feature 1: Personal Voice Engine (Moat) ---

pub fn observe_session(
    conn: &rusqlite::Connection,
    text: &str,
    tone: i32,
    formality: i32,
    _word_count: usize,
) -> rusqlite::Result<()> {
    // 1. Extract Opener (first word if alpha)
    let words: Vec<&str> = text.split_whitespace().collect();
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

    // 2. Extract Closer (last word if short)
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

    // 3. Vocab Fingerprinting (long words, unique)
    for word in words {
        let clean = word.trim_matches(|c: char| !c.is_alphabetic());
        if clean.len() > 6 {
            conn.execute(
                "INSERT INTO voice_profile (feature_type, feature_key, count) \
                 VALUES ('vocab', ?, 1) \
                 ON CONFLICT(feature_type, feature_key) DO UPDATE SET count = count + 1, last_seen = CURRENT_TIMESTAMP",
                [clean.to_lowercase()],
            )?;
        }
    }

    // 4. Formality & Tone averages
    conn.execute(
        "INSERT INTO voice_profile (feature_type, feature_key, value, count) \
         VALUES ('stat', 'formality', ?, 1) \
         ON CONFLICT(feature_type, feature_key) DO UPDATE SET value = ((CAST(value AS REAL) * count) + ?) / (count + 1), count = count + 1",
        [formality.to_string(), formality.to_string()],
    )?;

    conn.execute(
        "INSERT INTO voice_profile (feature_type, feature_key, value, count) \
         VALUES ('stat', 'tone', ?, 1) \
         ON CONFLICT(feature_type, feature_key) DO UPDATE SET value = ((CAST(value AS REAL) * count) + ?) / (count + 1), count = count + 1",
        [tone.to_string(), tone.to_string()],
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
// ── History read-back ──────────────────────────────────────────────────────

#[derive(Debug, serde::Serialize)]
pub struct HistoryEntry {
    pub id: i64,
    pub timestamp: String,
    pub input_preview: String,
    pub mode: String,
    pub output: String,
}

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