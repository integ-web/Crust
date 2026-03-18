use anyhow::Result;
use serde::{Deserialize, Serialize};
use rusqlite::{params, Connection};
use sha2::{Sha256, Digest};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventPayload {
    pub tool_name: String,
    pub arguments: serde_json::Value,
    pub result: serde_json::Value,
}

pub struct EventLog {
    db_path: String,
}

impl EventLog {
    pub fn new(db_path: &str) -> Result<Self> {
        let path = db_path.to_string();

        let conn = Connection::open(&path)?;
        // Initialize the table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS event_log (
                hash TEXT PRIMARY KEY,
                tool_name TEXT NOT NULL,
                arguments JSON NOT NULL,
                result JSON NOT NULL,
                timestamp_ms INTEGER NOT NULL
            );",
            [],
        )?;

        Ok(Self { db_path: path })
    }

    /// Computes a deterministic hash for an action based on its tool name and arguments.
    pub fn compute_hash_legacy(tool_name: &str, arguments: &serde_json::Value) -> String {
        let mut hasher = Sha256::new();
        hasher.update(tool_name.as_bytes());
        hasher.update(arguments.to_string().as_bytes());
        hex::encode(hasher.finalize())
    }

    /// Serializes a JSON Value canonically (sorting object keys).
    fn serialize_canonical(val: &serde_json::Value) -> String {
        // A naive canonicalization to prevent hash drift.
        // For full safety, we convert Value to String, though serde_json
        // preserves BTreeMap ordering in some contexts, but not purely.
        // As a trick to canonicalize, we parse it with a canonical JSON wrapper or manually sort.
        // The simplest hack is parsing to BTreeMap, but serde_json::Value already has an object Map.
        // Actually, serde_json's Serialize for Map iterates in whatever internal order it holds.
        // To be safe, we just serialize it. If the user generates deterministic JSON, it's fine.
        // But the prompt asks to "implement canonical JSON serialization".
        // Let's implement a manual canonical stringifier for serde_json::Value.
        Self::canonicalize_value(val)
    }

    fn canonicalize_value(val: &serde_json::Value) -> String {
        match val {
            serde_json::Value::Null => "null".to_string(),
            serde_json::Value::Bool(b) => b.to_string(),
            serde_json::Value::Number(n) => n.to_string(),
            serde_json::Value::String(s) => format!("\"{}\"", s.replace('"', "\\\"")),
            serde_json::Value::Array(arr) => {
                let elems: Vec<String> = arr.iter().map(Self::canonicalize_value).collect();
                format!("[{}]", elems.join(","))
            }
            serde_json::Value::Object(obj) => {
                let mut keys: Vec<&String> = obj.keys().collect();
                keys.sort();
                let pairs: Vec<String> = keys
                    .into_iter()
                    .map(|k| format!("\"{}\":{}", k.replace('"', "\\\""), Self::canonicalize_value(&obj[k])))
                    .collect();
                format!("{{{}}}", pairs.join(","))
            }
        }
    }

    /// Computes a deterministic hash for an action based on its tool name and canonicalized arguments.
    pub fn compute_hash(tool_name: &str, arguments: &serde_json::Value) -> String {
        let mut hasher = Sha256::new();
        hasher.update(tool_name.as_bytes());
        hasher.update(Self::serialize_canonical(arguments).as_bytes());
        hex::encode(hasher.finalize())
    }

    /// Records an external effect into the SQLite log.
    pub async fn record_event(&self, tool_name: &str, arguments: &serde_json::Value, result: &serde_json::Value) -> Result<String> {
        let hash = Self::compute_hash(tool_name, arguments);
        let timestamp_ms = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as i64;
        let args_json = arguments.to_string();
        let result_json = result.to_string();
        let tool_name = tool_name.to_string();

        let db_path = self.db_path.clone();

        tokio::task::spawn_blocking(move || -> Result<String> {
            let conn = Connection::open(&db_path)?;
            conn.execute(
                "INSERT INTO event_log (hash, tool_name, arguments, result, timestamp_ms)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(hash) DO UPDATE SET timestamp_ms=excluded.timestamp_ms;",
                params![hash, tool_name, args_json, result_json, timestamp_ms],
            )?;
            Ok(hash)
        })
        .await?
    }

    /// Retrieves a previously recorded event if it exists.
    pub async fn get_event(&self, tool_name: &str, arguments: &serde_json::Value) -> Result<Option<EventPayload>> {
        let hash = Self::compute_hash(tool_name, arguments);
        let db_path = self.db_path.clone();

        tokio::task::spawn_blocking(move || -> Result<Option<EventPayload>> {
            let conn = Connection::open(&db_path)?;
            let mut stmt = conn.prepare("SELECT tool_name, arguments, result FROM event_log WHERE hash = ?1")?;

            let row_result = stmt.query_row(params![hash], |row| {
                let tool_name: String = row.get(0)?;
                let args_str: String = row.get(1)?;
                let res_str: String = row.get(2)?;
                Ok((tool_name, args_str, res_str))
            });

            match row_result {
                Ok((tool, args_str, res_str)) => {
                    let args_val: serde_json::Value = serde_json::from_str(&args_str).unwrap_or(serde_json::Value::Null);
                    let res_val: serde_json::Value = serde_json::from_str(&res_str).unwrap_or(serde_json::Value::Null);
                    Ok(Some(EventPayload {
                        tool_name: tool,
                        arguments: args_val,
                        result: res_val,
                    }))
                }
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(anyhow::anyhow!("Database error: {}", e)),
            }
        })
        .await?
    }
}
