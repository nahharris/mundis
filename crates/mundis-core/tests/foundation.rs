use mundis_core::{
    config::{SimulationBias, SimulationConfig, WorldSize},
    export::{render_markdown, render_text},
    simulation::{Simulation, SimulationSeed},
    storage::SaveDatabase,
    world::World,
};

#[test]
fn same_seed_and_config_produce_identical_events_and_snapshot() {
    let config = SimulationConfig::default();
    let seed = SimulationSeed::from_u64(42);

    let mut first = Simulation::new(config.clone(), seed);
    let mut second = Simulation::new(config, seed);

    let first_events = first.run_months(6);
    let second_events = second.run_months(6);

    assert_eq!(first_events, second_events);
    assert_eq!(first.snapshot(), second.snapshot());
}

#[test]
fn generated_world_is_connected_and_inspectable() {
    let config = SimulationConfig {
        world: WorldSize { regions: 8 },
        ..SimulationConfig::default()
    };

    let world = World::generate(&config, SimulationSeed::from_u64(7));

    assert_eq!(world.regions.len(), 8);
    assert!(world.is_connected());
    assert!(world.regions.iter().all(|region| !region.name.is_empty()));
    assert!(
        world
            .regions
            .iter()
            .all(|region| !region.neighbors.is_empty())
    );
}

#[test]
fn config_round_trips_through_toml() {
    let config = SimulationConfig {
        months: 24,
        bias: SimulationBias::Dramatic,
        world: WorldSize { regions: 12 },
        ..SimulationConfig::default()
    };

    let toml = config.to_toml().expect("config serializes to TOML");
    let parsed = SimulationConfig::from_toml(&toml).expect("config parses from TOML");

    assert_eq!(parsed, config);
}

#[test]
fn markdown_is_a_renderer_over_structured_events() {
    let mut simulation = Simulation::new(SimulationConfig::default(), SimulationSeed::from_u64(11));
    let events = simulation.run_months(2);

    let text = render_text(&events);
    let markdown = render_markdown(&events);

    assert!(text.contains("Month 1"));
    assert!(markdown.starts_with("# Mundis Chronicle"));
    assert!(markdown.contains("## Year 1"));
    assert!(markdown.contains(&events[0].summary));
}

#[test]
fn save_database_persists_metadata_events_and_binary_snapshot() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let path = temp_dir.path().join("run.mundis.db");
    let config = SimulationConfig::default();
    let mut simulation = Simulation::new(config.clone(), SimulationSeed::from_u64(99));
    let events = simulation.run_months(3);
    let snapshot = simulation.snapshot();

    let db = SaveDatabase::create(&path, &config, SimulationSeed::from_u64(99)).expect("create db");
    db.append_events(&events).expect("append events");
    db.store_snapshot(&snapshot).expect("store snapshot");

    let reopened = SaveDatabase::open(&path).expect("open db");

    assert_eq!(reopened.load_config().expect("load config"), config);
    assert_eq!(reopened.load_events().expect("load events"), events);
    assert_eq!(
        reopened.load_latest_snapshot().expect("load snapshot"),
        snapshot
    );
}
