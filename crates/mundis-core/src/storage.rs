use std::{error::Error, path::Path};

use rusqlite::{Connection, OptionalExtension, params};

use crate::{
    config::SimulationConfig,
    simulation::{SimulationEvent, SimulationSeed, SimulationSnapshot},
};

const SCHEMA_VERSION: i64 = 1;

pub type StorageResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

pub struct SaveDatabase {
    connection: Connection,
}

impl SaveDatabase {
    pub fn create(
        path: &Path,
        config: &SimulationConfig,
        seed: SimulationSeed,
    ) -> StorageResult<Self> {
        let connection = Connection::open(path)?;
        let db = Self { connection };
        db.migrate()?;
        db.store_metadata(config, seed)?;
        Ok(db)
    }

    pub fn open(path: &Path) -> StorageResult<Self> {
        let connection = Connection::open(path)?;
        let db = Self { connection };
        let has_metadata_table = db
            .connection
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'metadata'",
                [],
                |_| Ok(()),
            )
            .optional()?
            .is_some();
        if !has_metadata_table {
            return Err("not a Mundis save database: missing metadata table".into());
        }

        let Some(version) = db
            .connection
            .query_row(
                "SELECT value FROM metadata WHERE key = 'schema_version'",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()?
        else {
            return Err("not a Mundis save database: missing schema_version".into());
        };
        let version = version.parse::<i64>()?;
        if version != SCHEMA_VERSION {
            return Err(format!("unsupported save schema version {version}").into());
        }
        Ok(db)
    }

    pub fn append_events(&self, events: &[SimulationEvent]) -> StorageResult<()> {
        for event in events {
            let payload = bincode::serde::encode_to_vec(event, bincode::config::standard())?;
            let severity = format!("{:?}", &event.severity);
            self.connection.execute(
                "INSERT INTO events (id, month, severity, summary, payload) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    event.id as i64,
                    event.month as i64,
                    severity,
                    &event.summary,
                    payload
                ],
            )?;
        }
        Ok(())
    }

    pub fn load_events(&self) -> StorageResult<Vec<SimulationEvent>> {
        let mut statement = self
            .connection
            .prepare("SELECT payload FROM events ORDER BY id ASC")?;
        let rows = statement.query_map([], |row| row.get::<_, Vec<u8>>(0))?;
        let mut events = Vec::new();

        for row in rows {
            let payload = row?;
            let (event, _) =
                bincode::serde::decode_from_slice(&payload, bincode::config::standard())?;
            events.push(event);
        }

        Ok(events)
    }

    pub fn store_snapshot(&self, snapshot: &SimulationSnapshot) -> StorageResult<()> {
        let payload = bincode::serde::encode_to_vec(snapshot, bincode::config::standard())?;
        self.connection.execute(
            "INSERT INTO snapshots (month, payload) VALUES (?1, ?2)",
            params![snapshot.state.month as i64, payload],
        )?;
        Ok(())
    }

    pub fn load_latest_snapshot(&self) -> StorageResult<SimulationSnapshot> {
        let payload: Vec<u8> = self.connection.query_row(
            "SELECT payload FROM snapshots ORDER BY month DESC LIMIT 1",
            [],
            |row| row.get(0),
        )?;
        let (snapshot, _) =
            bincode::serde::decode_from_slice(&payload, bincode::config::standard())?;
        Ok(snapshot)
    }

    pub fn load_config(&self) -> StorageResult<SimulationConfig> {
        let config_toml: String = self.connection.query_row(
            "SELECT value FROM metadata WHERE key = 'config_toml'",
            [],
            |row| row.get(0),
        )?;
        Ok(SimulationConfig::from_toml(&config_toml)?)
    }

    fn migrate(&self) -> StorageResult<()> {
        self.connection.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS metadata (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS events (
                id INTEGER PRIMARY KEY,
                month INTEGER NOT NULL,
                severity TEXT NOT NULL,
                summary TEXT NOT NULL,
                payload BLOB NOT NULL
            );
            CREATE TABLE IF NOT EXISTS snapshots (
                month INTEGER PRIMARY KEY,
                payload BLOB NOT NULL
            );
            ",
        )?;
        Ok(())
    }

    fn store_metadata(&self, config: &SimulationConfig, seed: SimulationSeed) -> StorageResult<()> {
        self.connection.execute(
            "INSERT OR REPLACE INTO metadata (key, value) VALUES ('schema_version', ?1)",
            params![SCHEMA_VERSION.to_string()],
        )?;
        self.connection.execute(
            "INSERT OR REPLACE INTO metadata (key, value) VALUES ('seed', ?1)",
            params![seed.value().to_string()],
        )?;
        self.connection.execute(
            "INSERT OR REPLACE INTO metadata (key, value) VALUES ('config_toml', ?1)",
            params![config.to_toml()?],
        )?;
        Ok(())
    }
}
