use rusqlite::{params, Connection, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Node {
    pub id: String,
    pub label: String,
    pub properties: String, // JSON representation
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Edge {
    pub id: String,
    pub source: String,
    pub target: String,
    pub relation: String,
    pub properties: String, // JSON representation
}

pub struct GraphDB {
    conn: Connection,
}

impl GraphDB {
    pub fn new(path: &str) -> Result<Self> {
        let conn = Connection::open(path)?;
        let mut db = Self { conn };
        db.init()?;
        Ok(db)
    }

    fn init(&mut self) -> Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute(
            "CREATE TABLE IF NOT EXISTS nodes (
                id TEXT PRIMARY KEY,
                label TEXT NOT NULL,
                properties TEXT NOT NULL
            )",
            [],
        )?;
        tx.execute(
            "CREATE TABLE IF NOT EXISTS edges (
                id TEXT PRIMARY KEY,
                source TEXT NOT NULL,
                target TEXT NOT NULL,
                relation TEXT NOT NULL,
                properties TEXT NOT NULL,
                FOREIGN KEY(source) REFERENCES nodes(id),
                FOREIGN KEY(target) REFERENCES nodes(id)
            )",
            [],
        )?;
        tx.execute(
            "CREATE INDEX IF NOT EXISTS idx_nodes_label ON nodes(label)",
            [],
        )?;
        tx.execute(
            "CREATE INDEX IF NOT EXISTS idx_edges_relation ON edges(relation)",
            [],
        )?;
        tx.execute(
            "CREATE INDEX IF NOT EXISTS idx_edges_source ON edges(source)",
            [],
        )?;
        tx.execute(
            "CREATE INDEX IF NOT EXISTS idx_edges_target ON edges(target)",
            [],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn insert_node(&mut self, label: &str, properties: &serde_json::Value) -> Result<String> {
        let id = Uuid::new_v4().to_string();
        let props_str = serde_json::to_string(properties).unwrap_or_else(|_| "{}".to_string());

        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT INTO nodes (id, label, properties) VALUES (?1, ?2, ?3)",
            params![id, label, props_str],
        )?;
        tx.commit()?;
        Ok(id)
    }

    pub fn insert_edge(
        &mut self,
        source: &str,
        target: &str,
        relation: &str,
        properties: &serde_json::Value,
    ) -> Result<String> {
        let id = Uuid::new_v4().to_string();
        let props_str = serde_json::to_string(properties).unwrap_or_else(|_| "{}".to_string());

        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT INTO edges (id, source, target, relation, properties) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, source, target, relation, props_str],
        )?;
        tx.commit()?;
        Ok(id)
    }

    pub fn get_node(&self, id: &str) -> Result<Option<Node>> {
        let mut stmt = self.conn.prepare("SELECT id, label, properties FROM nodes WHERE id = ?1")?;
        let mut rows = stmt.query(params![id])?;

        if let Some(row) = rows.next()? {
            Ok(Some(Node {
                id: row.get(0)?,
                label: row.get(1)?,
                properties: row.get(2)?,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn get_neighbors(&self, node_id: &str) -> Result<Vec<Node>> {
        let mut stmt = self.conn.prepare(
            "SELECT n.id, n.label, n.properties
             FROM nodes n
             JOIN edges e ON n.id = e.target
             WHERE e.source = ?1"
        )?;
        let rows = stmt.query_map(params![node_id], |row| {
            Ok(Node {
                id: row.get(0)?,
                label: row.get(1)?,
                properties: row.get(2)?,
            })
        })?;

        let mut neighbors = Vec::new();
        for node in rows {
            neighbors.push(node?);
        }
        Ok(neighbors)
    }
}
