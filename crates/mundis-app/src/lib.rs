use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow, bail};

use mundis_core::{
    config::{SimulationBias, SimulationConfig},
    export::{render_json, render_markdown, render_text},
    history::{AtlasState, HistoryQuery, SubjectFilter, atlas_state},
    scenario::ScenarioConfig,
    simulation::{
        EventSeverity, EventType, Simulation, SimulationEvent, SimulationSeed, SimulationSnapshot,
    },
    storage::SaveDatabase,
};
use serde::{Deserialize, Serialize};

pub type AppResult<T> = Result<T>;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateSimulationSettings {
    pub save_path: PathBuf,
    pub seed: u64,
    pub months: u32,
    pub regions: usize,
    pub initial_settlements: usize,
    pub initial_population_per_mille: u32,
    pub monthly_growth_per_mille: u32,
    pub civilization_enabled: bool,
    pub bias: SimulationBias,
}

#[derive(Clone, Debug)]
pub struct CreateSimulationRequest {
    pub seed: u64,
    pub config: SimulationConfig,
    pub base_config_toml: Option<String>,
    pub scenario_toml: Option<String>,
    pub save_path: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SimulationSummary {
    pub save_path: PathBuf,
    pub seed: u64,
    pub final_month: u32,
    pub event_count: usize,
    pub population: u64,
    pub regions: usize,
    pub settlements: usize,
    pub cultures: usize,
    pub polities: usize,
    pub events: Vec<SimulationEvent>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SimulationProgress {
    pub save_path: PathBuf,
    pub current_month: u32,
    pub total_months: u32,
    pub events_written: usize,
    pub population: u64,
}

#[derive(Clone, Debug)]
pub struct QueryEventsRequest {
    pub save_path: PathBuf,
    pub query: HistoryQuery,
}

#[derive(Clone, Debug, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExportFormat {
    Text,
    Markdown,
    Json,
}

pub fn create_simulation_from_settings(
    settings: CreateSimulationSettings,
) -> AppResult<SimulationSummary> {
    create_simulation_from_settings_with_progress(settings, |_| {})
}

pub fn create_simulation_from_settings_with_progress(
    settings: CreateSimulationSettings,
    on_progress: impl FnMut(SimulationProgress),
) -> AppResult<SimulationSummary> {
    let mut living_history = SimulationConfig::default().living_history;
    living_history.initial_settlements = settings.initial_settlements;
    living_history.initial_population_per_mille = settings.initial_population_per_mille;
    living_history.monthly_growth_per_mille = settings.monthly_growth_per_mille;

    let mut civilization = SimulationConfig::default().civilization;
    civilization.enabled = settings.civilization_enabled;

    let config = SimulationConfig {
        months: settings.months,
        world: mundis_core::config::WorldSize {
            regions: settings.regions,
        },
        civilization,
        living_history,
        bias: settings.bias,
        ..SimulationConfig::default()
    };
    let base_config_toml = config.to_toml().ok();

    create_simulation_with_progress(
        CreateSimulationRequest {
            seed: settings.seed,
            config,
            base_config_toml,
            scenario_toml: None,
            save_path: settings.save_path,
        },
        on_progress,
    )
}

pub fn create_simulation(request: CreateSimulationRequest) -> AppResult<SimulationSummary> {
    create_simulation_with_progress(request, |_| {})
}

pub fn create_simulation_with_progress(
    request: CreateSimulationRequest,
    mut on_progress: impl FnMut(SimulationProgress),
) -> AppResult<SimulationSummary> {
    let seed = SimulationSeed::from_u64(request.seed);
    let mut simulation = if let Some(scenario_toml) = request.scenario_toml.as_deref() {
        let scenario = ScenarioConfig::from_toml(scenario_toml)?;
        let compiled = scenario.compile(request.config.clone(), seed)?;
        Simulation::from_compiled_scenario(compiled)
    } else {
        Simulation::new(request.config.clone(), seed)
    };

    let db = convert_core_result(SaveDatabase::create_with_sources(
        &request.save_path,
        simulation.config(),
        seed,
        request.base_config_toml.as_deref(),
        request.scenario_toml.as_deref(),
    ))?;
    let events = run_and_save(&request.save_path, &mut simulation, &db, &mut on_progress)?;
    Ok(summary(
        request.save_path,
        request.seed,
        &simulation,
        events,
    ))
}

pub fn query_events(request: QueryEventsRequest) -> AppResult<Vec<SimulationEvent>> {
    let db = convert_core_result(SaveDatabase::open(&request.save_path))?;
    convert_core_result(db.query_events(&request.query))
}

pub fn entity_history(save_path: &Path, subject: SubjectFilter) -> AppResult<Vec<SimulationEvent>> {
    let db = convert_core_result(SaveDatabase::open(save_path))?;
    convert_core_result(db.entity_history(subject))
}

pub fn get_state_at_month(save_path: &Path, month: u32) -> AppResult<SimulationSnapshot> {
    let db = convert_core_result(SaveDatabase::open(save_path))?;
    convert_core_result(db.load_snapshot_at_month(month))
}

pub fn get_atlas_state(save_path: &Path, month: u32) -> AppResult<AtlasState> {
    Ok(atlas_state(&get_state_at_month(save_path, month)?))
}

pub fn load_events(save_path: &Path) -> AppResult<Vec<SimulationEvent>> {
    let db = convert_core_result(SaveDatabase::open(save_path))?;
    convert_core_result(db.load_events())
}

pub fn export_events(events: &[SimulationEvent], format: ExportFormat) -> AppResult<String> {
    match format {
        ExportFormat::Text => Ok(render_text(events)),
        ExportFormat::Markdown => Ok(render_markdown(events)),
        ExportFormat::Json => Ok(render_json(events)?),
    }
}

pub fn export_snapshot(snapshot: &SimulationSnapshot, format: ExportFormat) -> AppResult<String> {
    match format {
        ExportFormat::Text => Ok(format!(
            "Month {}: {} people, {} regions, {} settlements, {} cultures, {} polities\n",
            snapshot.state.month,
            snapshot.state.population,
            snapshot.state.world.regions.len(),
            snapshot.state.settlements.len(),
            snapshot.state.cultures.len(),
            snapshot.state.polities.len()
        )),
        ExportFormat::Markdown => Ok(format!(
            "# Mundis State\n\n- Month: {}\n- Population: {}\n- Regions: {}\n- Settlements: {}\n- Cultures: {}\n- Polities: {}\n",
            snapshot.state.month,
            snapshot.state.population,
            snapshot.state.world.regions.len(),
            snapshot.state.settlements.len(),
            snapshot.state.cultures.len(),
            snapshot.state.polities.len()
        )),
        ExportFormat::Json => Ok(serde_json::to_string_pretty(snapshot)?),
    }
}

pub fn parse_subject(input: &str) -> AppResult<SubjectFilter> {
    let Some((kind, id)) = input.split_once(':') else {
        bail!("expected subject like region:1 or polity:2");
    };
    let id = id.parse::<usize>()?;
    match kind {
        "region" => Ok(SubjectFilter::Region(id)),
        "settlement" => Ok(SubjectFilter::Settlement(id)),
        "population-group" => Ok(SubjectFilter::PopulationGroup(id)),
        "culture" => Ok(SubjectFilter::Culture(id)),
        "polity" => Ok(SubjectFilter::Polity(id)),
        _ => bail!("unknown subject kind '{kind}'"),
    }
}

pub fn parse_event_type(input: &str) -> AppResult<EventType> {
    match input {
        "settlement-founded" => Ok(EventType::SettlementFounded),
        "settlement-growth" => Ok(EventType::SettlementGrowth),
        "food-pressure" => Ok(EventType::FoodPressure),
        "migration" => Ok(EventType::Migration),
        "settlement-decline" => Ok(EventType::SettlementDecline),
        "environmental-stress" => Ok(EventType::EnvironmentalStress),
        "settlement-abandoned" => Ok(EventType::SettlementAbandoned),
        "polity-founded" => Ok(EventType::PolityFounded),
        "polity-expanded" => Ok(EventType::PolityExpanded),
        "trade-link-formed" => Ok(EventType::TradeLinkFormed),
        "border-tension" => Ok(EventType::BorderTension),
        "culture-drift" => Ok(EventType::CultureDrift),
        "polity-collapse" => Ok(EventType::PolityCollapse),
        "alliance-formed" => Ok(EventType::AllianceFormed),
        "war-declared" => Ok(EventType::WarDeclared),
        "war-ended" => Ok(EventType::WarEnded),
        "treaty-signed" => Ok(EventType::TreatySigned),
        "assimilation" => Ok(EventType::Assimilation),
        "revolt" => Ok(EventType::Revolt),
        "polity-fragmented" => Ok(EventType::PolityFragmented),
        "succession" => Ok(EventType::Succession),
        "background-event" => Ok(EventType::BackgroundEvent),
        _ => bail!("unknown event type '{input}'"),
    }
}

pub fn parse_severity(input: &str) -> AppResult<EventSeverity> {
    match input {
        "note" => Ok(EventSeverity::Note),
        "important" => Ok(EventSeverity::Important),
        _ => bail!("unknown severity '{input}'"),
    }
}

fn run_and_save(
    save_path: &Path,
    simulation: &mut Simulation,
    db: &SaveDatabase,
    on_progress: &mut impl FnMut(SimulationProgress),
) -> AppResult<Vec<SimulationEvent>> {
    let mut events = simulation.drain_pending_events();
    convert_core_result(db.store_snapshot(&simulation.snapshot()))?;
    if !events.is_empty() {
        convert_core_result(db.append_events(&events))?;
    }
    report_progress(save_path, simulation, events.len(), on_progress);

    for _ in 0..simulation.config().months {
        let month_events = simulation.tick_month();
        convert_core_result(db.append_events(&month_events))?;
        convert_core_result(db.store_snapshot(&simulation.snapshot()))?;
        events.extend(month_events);
        report_progress(save_path, simulation, events.len(), on_progress);
    }
    Ok(events)
}

fn report_progress(
    save_path: &Path,
    simulation: &Simulation,
    events_written: usize,
    on_progress: &mut impl FnMut(SimulationProgress),
) {
    let snapshot = simulation.snapshot();
    on_progress(SimulationProgress {
        save_path: save_path.to_path_buf(),
        current_month: snapshot.state.month,
        total_months: simulation.config().months,
        events_written,
        population: snapshot.state.population,
    });
}

fn convert_core_result<T>(
    result: std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>,
) -> AppResult<T> {
    result.map_err(|error| anyhow!(error.to_string()))
}

fn summary(
    save_path: PathBuf,
    seed: u64,
    simulation: &Simulation,
    events: Vec<SimulationEvent>,
) -> SimulationSummary {
    let snapshot = simulation.snapshot();
    SimulationSummary {
        save_path,
        seed,
        final_month: snapshot.state.month,
        event_count: events.len(),
        population: snapshot.state.population,
        regions: snapshot.state.world.regions.len(),
        settlements: snapshot.state.settlements.len(),
        cultures: snapshot.state.cultures.len(),
        polities: snapshot.state.polities.len(),
        events,
    }
}

#[cfg(test)]
mod tests {
    use mundis_core::{
        config::SimulationConfig,
        history::{HistoryQuery, SubjectFilter},
    };

    use crate::{
        CreateSimulationRequest, CreateSimulationSettings, ExportFormat, QueryEventsRequest,
        create_simulation, create_simulation_from_settings,
        create_simulation_from_settings_with_progress, export_events, get_atlas_state,
        query_events,
    };

    #[test]
    fn service_creates_queries_and_projects_a_simulation() {
        let temp = tempfile::tempdir().expect("temp dir");
        let save_path = temp.path().join("service.mundis");
        let config = SimulationConfig {
            months: 6,
            ..SimulationConfig::default()
        };

        let created = create_simulation(CreateSimulationRequest {
            seed: 7,
            config,
            base_config_toml: None,
            scenario_toml: None,
            save_path: save_path.clone(),
        })
        .expect("create simulation");

        assert_eq!(created.save_path, save_path);
        assert_eq!(created.final_month, 6);
        assert!(!created.events.is_empty());

        let atlas = get_atlas_state(&created.save_path, 3).expect("atlas state");
        assert_eq!(atlas.month, 3);

        let events = query_events(QueryEventsRequest {
            save_path: created.save_path,
            query: HistoryQuery {
                from_month: Some(1),
                to_month: Some(6),
                subject: Some(SubjectFilter::Settlement(0)),
                ..HistoryQuery::default()
            },
        })
        .expect("query events");
        assert!(!events.is_empty());

        let json = export_events(&events, ExportFormat::Json).expect("json export");
        assert!(json.contains("settlement-growth"));
    }

    #[test]
    fn service_creates_from_user_settings() {
        let temp = tempfile::tempdir().expect("temp dir");
        let save_path = temp.path().join("settings.mundis");

        let created = create_simulation_from_settings(CreateSimulationSettings {
            save_path,
            seed: 11,
            months: 4,
            regions: 5,
            initial_settlements: 3,
            initial_population_per_mille: 180,
            monthly_growth_per_mille: 12,
            civilization_enabled: true,
            bias: mundis_core::config::SimulationBias::Dramatic,
        })
        .expect("create simulation from settings");

        assert_eq!(created.final_month, 4);
        assert_eq!(created.regions, 5);
        assert!(created.settlements >= 3);
    }

    #[test]
    fn service_reports_progress_without_changing_created_summary() {
        let temp = tempfile::tempdir().expect("temp dir");
        let save_path = temp.path().join("progress.mundis");
        let settings = CreateSimulationSettings {
            save_path: save_path.clone(),
            seed: 17,
            months: 3,
            regions: 5,
            initial_settlements: 2,
            initial_population_per_mille: 220,
            monthly_growth_per_mille: 9,
            civilization_enabled: true,
            bias: mundis_core::config::SimulationBias::Plausible,
        };
        let mut progress = Vec::new();

        let created = create_simulation_from_settings_with_progress(settings.clone(), |update| {
            progress.push(update);
        })
        .expect("create simulation with progress");

        assert_eq!(created.final_month, 3);
        assert_eq!(
            progress
                .iter()
                .map(|update| update.current_month)
                .collect::<Vec<_>>(),
            vec![0, 1, 2, 3]
        );
        assert!(progress.iter().all(|update| update.save_path == save_path));
        assert!(progress.iter().all(|update| update.total_months == 3));
        assert_eq!(
            progress.last().expect("final progress").population,
            created.population
        );
        assert_eq!(
            progress.last().expect("final progress").events_written,
            created.event_count
        );
    }
}
