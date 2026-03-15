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
    conn: Connection,
}

impl EventLog {
    pub fn new(db_path: &str) -> Result<Self> {
        let conn = Connection::open(db_path)?;

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

        Ok(Self { conn })
    }

    /// Computes a deterministic hash for an action based on its tool name and arguments.
    pub fn compute_hash(tool_name: &str, arguments: &serde_json::Value) -> String {
        let mut hasher = Sha256::new();
        hasher.update(tool_name.as_bytes());
        hasher.update(arguments.to_string().as_bytes());
        hex::encode(hasher.finalize())
    }

    /// Records an external effect into the SQLite log.
    pub fn record_event(&self, tool_name: &str, arguments: &serde_json::Value, result: &serde_json::Value) -> Result<String> {
        let hash = Self::compute_hash(tool_name, arguments);
        let timestamp_ms = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as i64;

        let args_json = arguments.to_string();
        let result_json = result.to_string();

        self.conn.execute(
            "INSERT INTO event_log (hash, tool_name, arguments, result, timestamp_ms)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(hash) DO UPDATE SET timestamp_ms=excluded.timestamp_ms;",
            params![hash, tool_name, args_json, result_json, timestamp_ms],
        )?;

        Ok(hash)
    }

    /// Retrieves a previously recorded event if it exists.
    pub fn get_event(&self, tool_name: &str, arguments: &serde_json::Value) -> Result<Option<EventPayload>> {
        let hash = Self::compute_hash(tool_name, arguments);

        let mut stmt = self.conn.prepare("SELECT tool_name, arguments, result FROM event_log WHERE hash = ?1")?;

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
    }
}
