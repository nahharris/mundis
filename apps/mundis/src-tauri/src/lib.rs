use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

use mundis_app::{
    CreateSimulationSettings, QueryEventsRequest, SimulationProgress,
    create_simulation_from_settings_with_progress as service_create_simulation,
    get_atlas_state as service_get_atlas_state, get_causal_chain as service_get_causal_chain,
    parse_event_type, parse_severity, parse_subject, query_events as service_query_events,
};
use mundis_core::{
    config::SimulationBias,
    history::{AtlasState, CausalChain, HistoryQuery},
    simulation::SimulationEvent,
};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct FrontendLogInput {
    level: String,
    message: String,
    context: Option<serde_json::Value>,
    timestamp: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateSimulationInput {
    job_id: String,
    save_path: PathBuf,
    seed: u64,
    months: u32,
    regions: usize,
    initial_settlements: usize,
    initial_population_per_mille: u32,
    monthly_growth_per_mille: u32,
    civilization_enabled: bool,
    bias: SimulationBias,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct AppConfig {
    pub log_level: String,
    pub default_seed: u64,
    pub default_months: u32,
    pub default_regions: usize,
    pub default_settlements: usize,
    pub default_population_per_mille: u32,
    pub default_monthly_growth_per_mille: u32,
    pub default_civilization_enabled: bool,
    pub default_bias: SimulationBias,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            log_level: "info".to_string(),
            default_seed: 1,
            default_months: 120,
            default_regions: 8,
            default_settlements: 3,
            default_population_per_mille: 240,
            default_monthly_growth_per_mille: 8,
            default_civilization_enabled: true,
            default_bias: SimulationBias::Plausible,
        }
    }
}

impl AppConfig {
    fn from_toml(input: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(input)
    }

    fn to_toml(&self) -> Result<String, toml::ser::Error> {
        toml::to_string_pretty(self)
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MundisPathsOutput {
    home: PathBuf,
    saves_dir: PathBuf,
    logs_dir: PathBuf,
    frontend_log_file: PathBuf,
    backend_log_file: PathBuf,
    config_file: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct MundisPaths {
    home: PathBuf,
    saves_dir: PathBuf,
    logs_dir: PathBuf,
    opened_worlds_file: PathBuf,
    frontend_log_file: PathBuf,
    backend_log_file: PathBuf,
    config_file: PathBuf,
}

impl From<MundisPaths> for MundisPathsOutput {
    fn from(paths: MundisPaths) -> Self {
        Self {
            home: paths.home,
            saves_dir: paths.saves_dir,
            logs_dir: paths.logs_dir,
            frontend_log_file: paths.frontend_log_file,
            backend_log_file: paths.backend_log_file,
            config_file: paths.config_file,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SavePathOutput {
    name: String,
    path: PathBuf,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WorldSaveEntry {
    name: String,
    path: PathBuf,
    opened_at: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct WorldOpenRecord {
    path: String,
    opened_at: u64,
}

#[tauri::command]
fn get_mundis_paths() -> Result<MundisPathsOutput, String> {
    let paths = current_mundis_paths()?;
    ensure_mundis_layout(&paths)?;
    log_backend_event("backend.paths.loaded", None);
    Ok(paths.into())
}

#[tauri::command]
fn load_app_config() -> Result<AppConfig, String> {
    let paths = current_mundis_paths()?;
    ensure_mundis_layout(&paths)?;
    if !paths.config_file.exists() {
        let config = AppConfig::default();
        write_app_config(&paths, &config)?;
        log_backend_event("backend.config.created", None);
        return Ok(config);
    }

    let input = fs::read_to_string(&paths.config_file).map_err(|error| error.to_string())?;
    let config = AppConfig::from_toml(&input).map_err(|error| error.to_string())?;
    log_backend_event("backend.config.loaded", None);
    Ok(config)
}

#[tauri::command]
fn save_app_config(config: AppConfig) -> Result<AppConfig, String> {
    let paths = current_mundis_paths()?;
    ensure_mundis_layout(&paths)?;
    write_app_config(&paths, &config)?;
    log_backend_event("backend.config.saved", None);
    Ok(config)
}

#[tauri::command]
fn open_mundis_home() -> Result<(), String> {
    let paths = current_mundis_paths()?;
    ensure_mundis_layout(&paths)?;
    open_path_in_file_explorer(&paths.home)?;
    log_backend_event("backend.home.opened", None);
    Ok(())
}

#[tauri::command]
fn resolve_world_save_path(name: String) -> Result<SavePathOutput, String> {
    let paths = current_mundis_paths()?;
    ensure_mundis_layout(&paths)?;
    let stem = save_stem(&name)?;
    log_backend_event(
        "backend.world_save_path.resolved",
        Some(serde_json::json!({ "name": name, "stem": stem })),
    );
    Ok(SavePathOutput {
        name: display_name_from_stem(&stem),
        path: paths.saves_dir.join(format!("{stem}.mundis")),
    })
}

#[tauri::command]
fn list_world_saves() -> Result<Vec<WorldSaveEntry>, String> {
    let paths = current_mundis_paths()?;
    ensure_mundis_layout(&paths)?;
    let opened_worlds = load_opened_worlds(&paths);
    let mut saves = Vec::new();
    for entry in fs::read_dir(&paths.saves_dir).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("mundis") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        let opened_at = opened_worlds
            .iter()
            .find(|record| record.path == path_key(&path))
            .map(|record| record.opened_at);
        saves.push(WorldSaveEntry {
            name: display_name_from_stem(stem),
            path,
            opened_at,
        });
    }
    saves.sort_by(|left, right| right.opened_at.cmp(&left.opened_at));
    log_backend_event(
        "backend.world_saves.listed",
        Some(serde_json::json!({ "count": saves.len() })),
    );
    Ok(saves)
}

#[tauri::command]
fn record_world_opened(save_path: PathBuf) -> Result<(), String> {
    let paths = current_mundis_paths()?;
    ensure_mundis_layout(&paths)?;
    let save_path = validate_world_save_path(&paths, &save_path)?;
    remember_world_opened(&paths, &save_path, unix_timestamp())?;
    log_backend_event(
        "backend.world.opened",
        Some(serde_json::json!({ "save_path": save_path })),
    );
    Ok(())
}

#[tauri::command]
async fn create_simulation(
    app: AppHandle,
    input: CreateSimulationInput,
) -> Result<AtlasState, String> {
    let paths = current_mundis_paths()?;
    ensure_mundis_layout(&paths)?;
    let save_path = validate_world_save_path(&paths, &input.save_path)?;
    log_backend_event(
        "backend.simulation.create.start",
        Some(serde_json::json!({
            "save_path": save_path,
            "seed": input.seed,
            "months": input.months,
            "regions": input.regions
        })),
    );
    tauri::async_runtime::spawn_blocking(move || {
        let progress_app = app.clone();
        let job_id = input.job_id.clone();
        emit_progress(
            &progress_app,
            SimulationProgressEvent::initializing(job_id.clone(), save_path.clone(), input.months),
        );
        let summary = service_create_simulation(
            CreateSimulationSettings {
                save_path: save_path.clone(),
                seed: input.seed,
                months: input.months,
                regions: input.regions,
                initial_settlements: input.initial_settlements,
                initial_population_per_mille: input.initial_population_per_mille,
                monthly_growth_per_mille: input.monthly_growth_per_mille,
                civilization_enabled: input.civilization_enabled,
                bias: input.bias,
            },
            |progress| {
                emit_progress(
                    &progress_app,
                    SimulationProgressEvent::running(job_id.clone(), progress),
                );
            },
        )
        .map_err(|error| error.to_string())?;
        let atlas =
            service_get_atlas_state(&save_path, input.months).map_err(|error| error.to_string())?;
        let paths = current_mundis_paths()?;
        remember_world_opened(&paths, &save_path, unix_timestamp())?;
        emit_progress(
            &progress_app,
            SimulationProgressEvent::complete(
                job_id,
                save_path,
                input.months,
                summary.event_count,
                atlas.population,
            ),
        );
        log_backend_event(
            "backend.simulation.create.done",
            Some(serde_json::json!({
                "month": input.months,
                "events": summary.event_count,
                "population": atlas.population
            })),
        );
        Ok(atlas)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SimulationProgressEvent {
    job_id: String,
    save_path: PathBuf,
    current_month: u32,
    total_months: u32,
    events_written: usize,
    population: u64,
    stage: SimulationProgressStage,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
enum SimulationProgressStage {
    Initializing,
    Running,
    Complete,
}

impl SimulationProgressEvent {
    fn initializing(job_id: String, save_path: PathBuf, total_months: u32) -> Self {
        Self {
            job_id,
            save_path,
            current_month: 0,
            total_months,
            events_written: 0,
            population: 0,
            stage: SimulationProgressStage::Initializing,
        }
    }

    fn running(job_id: String, progress: SimulationProgress) -> Self {
        Self {
            job_id,
            save_path: progress.save_path,
            current_month: progress.current_month,
            total_months: progress.total_months,
            events_written: progress.events_written,
            population: progress.population,
            stage: SimulationProgressStage::Running,
        }
    }

    fn complete(
        job_id: String,
        save_path: PathBuf,
        total_months: u32,
        events_written: usize,
        population: u64,
    ) -> Self {
        Self {
            job_id,
            save_path,
            current_month: total_months,
            total_months,
            events_written,
            population,
            stage: SimulationProgressStage::Complete,
        }
    }
}

fn emit_progress(app: &AppHandle, event: SimulationProgressEvent) {
    let _ = app.emit("simulation-progress", event);
}

#[tauri::command]
fn record_frontend_log(
    level: String,
    message: String,
    context: Option<serde_json::Value>,
    timestamp: String,
) -> Result<(), String> {
    let input = FrontendLogInput {
        level,
        message,
        context,
        timestamp,
    };
    write_frontend_log(&input)
}

#[tauri::command]
async fn get_atlas_state(save_path: String, month: u32) -> Result<AtlasState, String> {
    let paths = current_mundis_paths()?;
    ensure_mundis_layout(&paths)?;
    let save_path = validate_world_save_path(&paths, Path::new(&save_path))?;
    log_backend_event(
        "backend.atlas.load.start",
        Some(serde_json::json!({ "save_path": save_path, "month": month })),
    );
    tauri::async_runtime::spawn_blocking(move || {
        service_get_atlas_state(&save_path, month).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn query_events(
    save_path: String,
    from_month: Option<u32>,
    to_month: Option<u32>,
    tag: Option<String>,
    subject: Option<String>,
    event_type: Option<String>,
    severity: Option<String>,
) -> Result<Vec<SimulationEvent>, String> {
    let paths = current_mundis_paths()?;
    ensure_mundis_layout(&paths)?;
    let save_path = validate_world_save_path(&paths, Path::new(&save_path))?;
    log_backend_event(
        "backend.events.query.start",
        Some(serde_json::json!({
            "save_path": save_path,
            "from_month": from_month,
            "to_month": to_month,
            "tag": tag,
            "subject": subject,
            "event_type": event_type,
            "severity": severity
        })),
    );
    tauri::async_runtime::spawn_blocking(move || {
        let query = HistoryQuery {
            from_month,
            to_month,
            tag,
            subject: subject
                .as_deref()
                .map(parse_subject)
                .transpose()
                .map_err(|error| error.to_string())?,
            event_type: event_type
                .as_deref()
                .map(parse_event_type)
                .transpose()
                .map_err(|error| error.to_string())?,
            severity: severity
                .as_deref()
                .map(parse_severity)
                .transpose()
                .map_err(|error| error.to_string())?,
        };

        service_query_events(QueryEventsRequest {
            save_path,
            query,
        })
        .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn get_causal_chain(
    save_path: String,
    event_id: u64,
    depth: Option<u32>,
) -> Result<CausalChain, String> {
    let paths = current_mundis_paths()?;
    ensure_mundis_layout(&paths)?;
    let save_path = validate_world_save_path(&paths, Path::new(&save_path))?;
    let depth = depth.unwrap_or(2);
    tauri::async_runtime::spawn_blocking(move || {
        service_get_causal_chain(&save_path, event_id, depth)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            create_simulation,
            get_mundis_paths,
            load_app_config,
            save_app_config,
            open_mundis_home,
            resolve_world_save_path,
            list_world_saves,
            record_world_opened,
            record_frontend_log,
            get_atlas_state,
            query_events,
            get_causal_chain
        ])
        .run(tauri::generate_context!())
        .expect("error while running Mundis");
}

fn current_mundis_paths() -> Result<MundisPaths, String> {
    Ok(mundis_paths_from_home(
        &mundis_home_dir().map_err(|error| error.to_string())?,
    ))
}

fn mundis_paths_from_home(home: &Path) -> MundisPaths {
    let home = home.to_path_buf();
    let saves_dir = home.join("saves");
    let logs_dir = home.join("logs");
    MundisPaths {
        config_file: home.join("config.toml"),
        saves_dir,
        opened_worlds_file: home.join("opened-worlds.json"),
        frontend_log_file: logs_dir.join("frontend.jsonl"),
        backend_log_file: logs_dir.join("backend.jsonl"),
        logs_dir,
        home,
    }
}

fn ensure_mundis_layout(paths: &MundisPaths) -> Result<(), String> {
    fs::create_dir_all(&paths.home).map_err(|error| error.to_string())?;
    fs::create_dir_all(&paths.saves_dir).map_err(|error| error.to_string())?;
    fs::create_dir_all(&paths.logs_dir).map_err(|error| error.to_string())
}

fn write_app_config(paths: &MundisPaths, config: &AppConfig) -> Result<(), String> {
    let encoded = config.to_toml().map_err(|error| error.to_string())?;
    fs::write(&paths.config_file, encoded).map_err(|error| error.to_string())
}

fn write_frontend_log(input: &FrontendLogInput) -> Result<(), String> {
    let paths = current_mundis_paths()?;
    ensure_mundis_layout(&paths)?;
    let line = serde_json::to_string(input).map_err(|error| error.to_string())?;
    append_log_line(&paths.frontend_log_file, &line)
}

fn remember_world_opened(paths: &MundisPaths, save_path: &Path, opened_at: u64) -> Result<(), String> {
    let key = path_key(save_path);
    let mut records = load_opened_worlds(paths);
    records.retain(|record| record.path != key);
    records.push(WorldOpenRecord {
        path: key,
        opened_at,
    });
    records.sort_by(|left, right| right.opened_at.cmp(&left.opened_at));
    let encoded = serde_json::to_string_pretty(&records).map_err(|error| error.to_string())?;
    fs::write(&paths.opened_worlds_file, encoded).map_err(|error| error.to_string())
}

fn load_opened_worlds(paths: &MundisPaths) -> Vec<WorldOpenRecord> {
    let Ok(input) = fs::read_to_string(&paths.opened_worlds_file) else {
        return Vec::new();
    };
    serde_json::from_str(&input).unwrap_or_default()
}

fn path_key(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

fn validate_world_save_path(paths: &MundisPaths, save_path: &Path) -> Result<PathBuf, String> {
    if save_path.extension().and_then(|extension| extension.to_str()) != Some("mundis") {
        return Err("save path must use the .mundis extension".to_string());
    }

    let saves_dir = paths
        .saves_dir
        .canonicalize()
        .map_err(|error| error.to_string())?;

    let resolved = if save_path.exists() {
        save_path.canonicalize().map_err(|error| error.to_string())?
    } else {
        let Some(parent) = save_path.parent() else {
            return Err("save path must include a parent directory".to_string());
        };
        let file_name = save_path
            .file_name()
            .ok_or_else(|| "save path must include a file name".to_string())?;
        let parent = parent.canonicalize().map_err(|error| error.to_string())?;
        parent.join(file_name)
    };

    if !resolved.starts_with(&saves_dir) {
        return Err("save path must be inside the Mundis saves directory".to_string());
    }

    Ok(resolved)
}

fn log_backend_event(message: &str, context: Option<serde_json::Value>) {
    if let Ok(paths) = current_mundis_paths() {
        let _ = ensure_mundis_layout(&paths);
        let record = serde_json::json!({
            "level": "info",
            "message": message,
            "context": context,
            "timestamp": unix_timestamp(),
        });
        let _ = append_log_line(&paths.backend_log_file, &record.to_string());
    }
}

fn append_log_line(path: &Path, line: &str) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|error| error.to_string())?;
    writeln!(file, "{line}").map_err(|error| error.to_string())
}

fn unix_timestamp() -> u64 {
    UNIX_EPOCH.elapsed().map_or(0, |duration| duration.as_secs())
}

fn open_path_in_file_explorer(path: &Path) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = std::process::Command::new("explorer");
        command.arg(path);
        command
    };

    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = std::process::Command::new("open");
        command.arg(path);
        command
    };

    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = {
        let mut command = std::process::Command::new("xdg-open");
        command.arg(path);
        command
    };

    command.spawn().map_err(|error| error.to_string())?;
    Ok(())
}

fn mundis_home_dir() -> Result<PathBuf, &'static str> {
    if let Some(path) = std::env::var_os("MUNDIS_HOME").filter(|path| !path.is_empty()) {
        return Ok(PathBuf::from(path));
    }

    if let Some(path) = std::env::var_os("USERPROFILE").filter(|path| !path.is_empty()) {
        return Ok(PathBuf::from(path).join(".mundis"));
    }

    if let Some(path) = std::env::var_os("HOME").filter(|path| !path.is_empty()) {
        return Ok(PathBuf::from(path).join(".mundis"));
    }

    Err("could not resolve Mundis save directory; set MUNDIS_HOME")
}

fn save_stem(name: &str) -> Result<String, String> {
    let mut stem = String::new();
    let mut previous_was_separator = true;
    for character in name.trim().chars() {
        if character.is_ascii_alphanumeric() {
            stem.push(character.to_ascii_lowercase());
            previous_was_separator = false;
        } else if !previous_was_separator {
            stem.push('-');
            previous_was_separator = true;
        }
    }
    while stem.ends_with('-') {
        stem.pop();
    }
    if stem.is_empty() {
        return Err("world name must contain at least one letter or number".to_string());
    }
    Ok(stem)
}

fn display_name_from_stem(stem: &str) -> String {
    stem.split('-')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut characters = part.chars();
            let Some(first) = characters.next() else {
                return String::new();
            };
            format!("{}{}", first.to_ascii_uppercase(), characters.as_str())
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{
        AppConfig, display_name_from_stem, mundis_paths_from_home, save_stem,
        validate_world_save_path,
    };

    #[test]
    fn save_stem_normalizes_world_names_for_files() {
        assert_eq!(save_stem("  First Age!  ").expect("stem"), "first-age");
        assert_eq!(save_stem("World_42").expect("stem"), "world-42");
        assert!(save_stem(" ... ").is_err());
    }

    #[test]
    fn validate_world_save_path_rejects_paths_outside_saves_dir() {
        let temp = tempfile::tempdir().expect("temp dir");
        let home = temp.path().join("home");
        let paths = mundis_paths_from_home(&home);
        std::fs::create_dir_all(&paths.saves_dir).expect("saves dir");

        let allowed = paths.saves_dir.join("allowed.mundis");
        assert!(validate_world_save_path(&paths, &allowed).is_ok());

        let outside = temp.path().join("outside.mundis");
        assert!(validate_world_save_path(&paths, &outside).is_err());
    }

    #[test]
    fn display_name_is_derived_from_save_stem() {
        assert_eq!(display_name_from_stem("first-age"), "First Age");
        assert_eq!(display_name_from_stem("world-42"), "World 42");
    }

    #[test]
    fn mundis_paths_keep_logs_and_config_inside_home() {
        let paths = mundis_paths_from_home(Path::new("C:/MundisHome"));

        assert_eq!(paths.config_file, Path::new("C:/MundisHome").join("config.toml"));
        assert_eq!(paths.saves_dir, Path::new("C:/MundisHome").join("saves"));
        assert_eq!(paths.logs_dir, Path::new("C:/MundisHome").join("logs"));
        assert_eq!(
            paths.opened_worlds_file,
            Path::new("C:/MundisHome").join("opened-worlds.json")
        );
        assert_eq!(
            paths.frontend_log_file,
            Path::new("C:/MundisHome").join("logs").join("frontend.jsonl")
        );
        assert_eq!(
            paths.backend_log_file,
            Path::new("C:/MundisHome").join("logs").join("backend.jsonl")
        );
    }

    #[test]
    fn app_config_round_trips_as_toml() {
        let config = AppConfig::default();
        let encoded = config.to_toml().expect("encode config");
        let decoded = AppConfig::from_toml(&encoded).expect("decode config");

        assert_eq!(decoded, config);
        assert!(encoded.contains("log_level"));
        assert!(encoded.contains("default_months"));
    }
}
