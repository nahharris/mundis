use std::{error::Error, path::Path};

use rusqlite::{Connection, OptionalExtension, params, params_from_iter};

use crate::{
    config::SimulationConfig,
    history::{CausalChain, HistoryQuery, SubjectFilter, event_type_key, severity_key},
    simulation::{SimulationEvent, SimulationSeed, SimulationSnapshot},
};

const SCHEMA_VERSION: i64 = 2;

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
        reject_existing_path(path)?;
        let connection = open_connection(path)?;
        let db = Self { connection };
        db.initialize_schema()?;
        db.store_metadata(config, seed, None, None)?;
        Ok(db)
    }

    pub fn create_with_sources(
        path: &Path,
        config: &SimulationConfig,
        seed: SimulationSeed,
        base_config_toml: Option<&str>,
        scenario_toml: Option<&str>,
    ) -> StorageResult<Self> {
        reject_existing_path(path)?;
        let connection = open_connection(path)?;
        let db = Self { connection };
        db.initialize_schema()?;
        db.store_metadata(config, seed, base_config_toml, scenario_toml)?;
        Ok(db)
    }

    pub fn open(path: &Path) -> StorageResult<Self> {
        let connection = open_connection(path)?;
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
        let transaction = self.connection.unchecked_transaction()?;
        {
            let mut insert_event = transaction.prepare(
                "INSERT INTO events (id, month, event_type, severity, summary, payload) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )?;
            let mut insert_tag = transaction
                .prepare("INSERT INTO event_tags (event_id, tag) VALUES (?1, ?2)")?;
            let mut insert_subject = transaction.prepare(
                "INSERT INTO event_subjects (event_id, subject) VALUES (?1, ?2)",
            )?;
            let mut insert_link = transaction.prepare(
                "INSERT INTO event_links (cause_id, effect_id) VALUES (?1, ?2)",
            )?;

            for event in events {
                let payload = bincode::serde::encode_to_vec(event, bincode::config::standard())?;
                let event_type = event_type_key(&event.event_type);
                let severity = severity_key(&event.severity);
                insert_event.execute(params![
                    event.id as i64,
                    event.month as i64,
                    event_type,
                    severity,
                    &event.summary,
                    payload
                ])?;
                for tag in &event.tags {
                    insert_tag
                        .execute(params![event.id as i64, tag])?;
                }
                for subject in &event.subjects {
                    let subject = SubjectFilter::from(subject).key();
                    insert_subject
                        .execute(params![event.id as i64, subject])?;
                }
                for cause_id in &event.caused_by {
                    insert_link.execute(params![*cause_id as i64, event.id as i64])?;
                }
            }
        }
        transaction.commit()?;
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
            "INSERT OR REPLACE INTO snapshots (month, payload) VALUES (?1, ?2)",
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

    pub fn load_snapshot_at_month(&self, month: u32) -> StorageResult<SimulationSnapshot> {
        let payload: Vec<u8> = self.connection.query_row(
            "SELECT payload FROM snapshots WHERE month = ?1",
            params![month as i64],
            |row| row.get(0),
        )?;
        let (snapshot, _) =
            bincode::serde::decode_from_slice(&payload, bincode::config::standard())?;
        Ok(snapshot)
    }

    pub fn latest_snapshot_month(&self) -> StorageResult<u32> {
        let month: i64 = self.connection.query_row(
            "SELECT month FROM snapshots ORDER BY month DESC LIMIT 1",
            [],
            |row| row.get(0),
        )?;
        Ok(month as u32)
    }

    pub fn load_nearest_snapshot_at_or_before(
        &self,
        month: u32,
    ) -> StorageResult<SimulationSnapshot> {
        let payload: Option<Vec<u8>> = self
            .connection
            .query_row(
                "SELECT payload FROM snapshots WHERE month <= ?1 ORDER BY month DESC LIMIT 1",
                params![month as i64],
                |row| row.get(0),
            )
            .optional()?;
        let Some(payload) = payload else {
            return Err(format!("no snapshot at or before month {month}").into());
        };
        let (snapshot, _) =
            bincode::serde::decode_from_slice(&payload, bincode::config::standard())?;
        Ok(snapshot)
    }

    pub fn ensure_reconstructible_month(&self, month: u32) -> StorageResult<()> {
        if month > self.latest_snapshot_month()? {
            return Err(format!(
                "month {month} is after the saved simulation horizon"
            )
            .into());
        }
        self.load_nearest_snapshot_at_or_before(month)?;
        Ok(())
    }

    pub fn load_event_by_id(&self, event_id: u64) -> StorageResult<SimulationEvent> {
        let payload: Vec<u8> = self.connection.query_row(
            "SELECT payload FROM events WHERE id = ?1",
            params![event_id as i64],
            |row| row.get(0),
        )?;
        let (event, _) =
            bincode::serde::decode_from_slice(&payload, bincode::config::standard())?;
        Ok(event)
    }

    pub fn load_causal_chain(&self, event_id: u64, depth: u32) -> StorageResult<CausalChain> {
        let event = self.load_event_by_id(event_id)?;
        let mut causes = Vec::new();
        let mut effects = Vec::new();
        let mut cause_frontier = event.caused_by.clone();
        let mut effect_frontier = vec![event_id];
        let depth = depth.max(1);

        for _ in 0..depth {
            let mut next_causes = Vec::new();
            for id in cause_frontier.drain(..) {
                if causes.iter().any(|event: &SimulationEvent| event.id == id) {
                    continue;
                }
                let cause = self.load_event_by_id(id)?;
                next_causes.extend(cause.caused_by.iter().copied());
                causes.push(cause);
            }
            cause_frontier = next_causes;
            if cause_frontier.is_empty() {
                break;
            }
        }

        for _ in 0..depth {
            let mut next_effects = Vec::new();
            for id in effect_frontier.drain(..) {
                let mut statement = self.connection.prepare(
                    "SELECT payload FROM events e
                     INNER JOIN event_links l ON l.effect_id = e.id
                     WHERE l.cause_id = ?1
                     ORDER BY e.id ASC",
                )?;
                let rows = statement.query_map(params![id as i64], |row| row.get::<_, Vec<u8>>(0))?;
                for row in rows {
                    let payload = row?;
                    let (effect, _): (SimulationEvent, usize) =
                        bincode::serde::decode_from_slice(&payload, bincode::config::standard())?;
                    if effects
                        .iter()
                        .any(|stored: &SimulationEvent| stored.id == effect.id)
                    {
                        continue;
                    }
                    next_effects.push(effect.id);
                    effects.push(effect);
                }
            }
            effect_frontier = next_effects;
            if effect_frontier.is_empty() {
                break;
            }
        }

        causes.sort_by_key(|event| event.id);
        effects.sort_by_key(|event| event.id);
        Ok(CausalChain {
            event,
            causes,
            effects,
        })
    }

    pub fn query_events(&self, query: &HistoryQuery) -> StorageResult<Vec<SimulationEvent>> {
        let mut sql = String::from("SELECT DISTINCT e.payload FROM events e");
        if query.tag.is_some() {
            sql.push_str(" INNER JOIN event_tags et ON et.event_id = e.id");
        }
        if query.subject.is_some() {
            sql.push_str(" INNER JOIN event_subjects es ON es.event_id = e.id");
        }

        let mut conditions = Vec::new();
        let mut bind_values: Vec<rusqlite::types::Value> = Vec::new();

        if let Some(from_month) = query.from_month {
            conditions.push("e.month >= ?");
            bind_values.push((from_month as i64).into());
        }
        if let Some(to_month) = query.to_month {
            conditions.push("e.month <= ?");
            bind_values.push((to_month as i64).into());
        }
        if let Some(tag) = &query.tag {
            conditions.push("et.tag = ?");
            bind_values.push(tag.clone().into());
        }
        if let Some(subject) = query.subject {
            conditions.push("es.subject = ?");
            bind_values.push(subject.key().into());
        }
        if let Some(event_type) = &query.event_type {
            conditions.push("e.event_type = ?");
            bind_values.push(event_type_key(event_type).into());
        }
        if let Some(severity) = &query.severity {
            conditions.push("e.severity = ?");
            bind_values.push(severity_key(severity).into());
        }

        if !conditions.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&conditions.join(" AND "));
        }
        sql.push_str(" ORDER BY e.id ASC");

        let mut statement = self.connection.prepare(&sql)?;
        let rows = statement.query_map(params_from_iter(bind_values), |row| {
            row.get::<_, Vec<u8>>(0)
        })?;
        let mut events = Vec::new();
        for row in rows {
            let payload = row?;
            let (event, _) =
                bincode::serde::decode_from_slice(&payload, bincode::config::standard())?;
            events.push(event);
        }
        Ok(events)
    }

    pub fn entity_history(&self, subject: SubjectFilter) -> StorageResult<Vec<SimulationEvent>> {
        self.query_events(&HistoryQuery {
            subject: Some(subject),
            ..HistoryQuery::default()
        })
    }

    pub fn load_config(&self) -> StorageResult<SimulationConfig> {
        let config_toml: String = self.connection.query_row(
            "SELECT value FROM metadata WHERE key = 'config_toml'",
            [],
            |row| row.get(0),
        )?;
        Ok(SimulationConfig::from_toml(&config_toml)?)
    }

    pub fn load_base_config_source(&self) -> StorageResult<Option<String>> {
        self.load_optional_metadata("base_config_toml")
    }

    pub fn load_scenario_source(&self) -> StorageResult<Option<String>> {
        self.load_optional_metadata("scenario_toml")
    }

    fn initialize_schema(&self) -> StorageResult<()> {
        self.connection.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS metadata (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS events (
                id INTEGER PRIMARY KEY,
                month INTEGER NOT NULL,
                event_type TEXT NOT NULL,
                severity TEXT NOT NULL,
                summary TEXT NOT NULL,
                payload BLOB NOT NULL
            );
            CREATE TABLE IF NOT EXISTS event_tags (
                event_id INTEGER NOT NULL,
                tag TEXT NOT NULL,
                PRIMARY KEY (event_id, tag),
                FOREIGN KEY (event_id) REFERENCES events(id) ON DELETE CASCADE
            );
            CREATE TABLE IF NOT EXISTS event_subjects (
                event_id INTEGER NOT NULL,
                subject TEXT NOT NULL,
                PRIMARY KEY (event_id, subject),
                FOREIGN KEY (event_id) REFERENCES events(id) ON DELETE CASCADE
            );
            CREATE TABLE IF NOT EXISTS snapshots (
                month INTEGER PRIMARY KEY,
                payload BLOB NOT NULL
            );
            CREATE TABLE IF NOT EXISTS event_links (
                cause_id INTEGER NOT NULL,
                effect_id INTEGER NOT NULL,
                PRIMARY KEY (cause_id, effect_id),
                FOREIGN KEY (cause_id) REFERENCES events(id) ON DELETE CASCADE,
                FOREIGN KEY (effect_id) REFERENCES events(id) ON DELETE CASCADE
            );
            CREATE INDEX IF NOT EXISTS idx_events_month ON events(month);
            CREATE INDEX IF NOT EXISTS idx_events_type ON events(event_type);
            CREATE INDEX IF NOT EXISTS idx_events_severity ON events(severity);
            CREATE INDEX IF NOT EXISTS idx_event_tags_tag ON event_tags(tag);
            CREATE INDEX IF NOT EXISTS idx_event_subjects_subject ON event_subjects(subject);
            CREATE INDEX IF NOT EXISTS idx_event_links_cause ON event_links(cause_id);
            CREATE INDEX IF NOT EXISTS idx_event_links_effect ON event_links(effect_id);
            ",
        )?;
        Ok(())
    }

    fn store_metadata(
        &self,
        config: &SimulationConfig,
        seed: SimulationSeed,
        base_config_toml: Option<&str>,
        scenario_toml: Option<&str>,
    ) -> StorageResult<()> {
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
        if let Some(base_config_toml) = base_config_toml {
            self.connection.execute(
                "INSERT INTO metadata (key, value) VALUES ('base_config_toml', ?1)",
                params![base_config_toml],
            )?;
        }
        if let Some(scenario_toml) = scenario_toml {
            self.connection.execute(
                "INSERT INTO metadata (key, value) VALUES ('scenario_toml', ?1)",
                params![scenario_toml],
            )?;
        }
        Ok(())
    }

    fn load_optional_metadata(&self, key: &str) -> StorageResult<Option<String>> {
        Ok(self
            .connection
            .query_row(
                "SELECT value FROM metadata WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()?)
    }
}

fn open_connection(path: &Path) -> StorageResult<Connection> {
    let connection = Connection::open(path)?;
    connection.execute_batch("PRAGMA foreign_keys = ON;")?;
    Ok(connection)
}

fn reject_existing_path(path: &Path) -> StorageResult<()> {
    if path.exists() {
        return Err(format!("save database already exists at {}", path.display()).into());
    }
    Ok(())
}
