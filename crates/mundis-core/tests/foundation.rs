use mundis_core::{
    config::{SimulationBias, SimulationConfig, WorldSize},
    export::{render_json, render_markdown, render_text},
    simulation::{EventSeverity, Simulation, SimulationEvent, SimulationSeed},
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

    for (expected_id, region) in world.regions.iter().enumerate() {
        assert_eq!(region.id, expected_id);
        assert!(!region.name.is_empty());
        assert!(!region.resources.is_empty());
        assert!(!region.neighbors.is_empty());
        assert!(
            region
                .neighbors
                .windows(2)
                .all(|window| window[0] < window[1])
        );
        assert!(
            region
                .neighbors
                .iter()
                .all(|neighbor| *neighbor < world.regions.len())
        );
    }
}

#[test]
fn simulation_events_reference_valid_months_and_snapshot_state() {
    let config = SimulationConfig {
        months: 18,
        world: WorldSize { regions: 5 },
        ..SimulationConfig::default()
    };
    let mut simulation = Simulation::new(config.clone(), SimulationSeed::from_u64(27));

    let events = simulation.run_months(config.months);
    let snapshot = simulation.snapshot();

    assert_eq!(events.len(), config.months as usize);
    for (index, event) in events.iter().enumerate() {
        assert_eq!(event.id, index as u64 + 1);
        assert_eq!(event.month, index as u32 + 1);
        assert!(!event.tags.is_empty());
        assert!(!event.summary.is_empty());
    }
    assert_eq!(snapshot.state.month, config.months);
    assert_eq!(snapshot.state.event_count, config.months as u64);
    assert!(snapshot.state.world.is_connected());
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
fn text_json_and_markdown_exports_are_stable_renderers() {
    let events = vec![
        SimulationEvent {
            id: 1,
            month: 1,
            severity: EventSeverity::Note,
            tags: vec!["population".to_string()],
            summary: "Aruven stirred as terraces filled.".to_string(),
        },
        SimulationEvent {
            id: 2,
            month: 12,
            severity: EventSeverity::Important,
            tags: vec!["region".to_string(), "politics".to_string()],
            summary: "Beltor reshaped its border customs.".to_string(),
        },
    ];

    assert_eq!(
        render_text(&events),
        "Month 1: Aruven stirred as terraces filled.\nMonth 12: Beltor reshaped its border customs.\n"
    );
    assert_eq!(
        render_markdown(&events),
        "# Mundis Chronicle\n\n## Year 1\n- Month 1: Aruven stirred as terraces filled.\n- Month 12: Beltor reshaped its border customs.\n"
    );
    assert_eq!(
        render_json(&events).expect("JSON renders"),
        "[\n  {\n    \"id\": 1,\n    \"month\": 1,\n    \"severity\": \"note\",\n    \"tags\": [\n      \"population\"\n    ],\n    \"summary\": \"Aruven stirred as terraces filled.\"\n  },\n  {\n    \"id\": 2,\n    \"month\": 12,\n    \"severity\": \"important\",\n    \"tags\": [\n      \"region\",\n      \"politics\"\n    ],\n    \"summary\": \"Beltor reshaped its border customs.\"\n  }\n]"
    );
}

#[test]
fn markdown_handles_zero_month_events_without_underflowing() {
    let events = vec![SimulationEvent {
        id: 1,
        month: 0,
        severity: EventSeverity::Note,
        tags: vec!["malformed".to_string()],
        summary: "A malformed event still renders safely.".to_string(),
    }];

    let markdown = render_markdown(&events);

    assert!(markdown.contains("## Year 1"));
    assert!(markdown.contains("Month 0"));
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

#[test]
fn opening_non_mundis_database_reports_clear_error() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let path = temp_dir.path().join("not-mundis.db");
    let connection = rusqlite::Connection::open(&path).expect("create sqlite db");
    connection
        .execute("CREATE TABLE unrelated (id INTEGER PRIMARY KEY)", [])
        .expect("create unrelated table");
    drop(connection);

    let error = SaveDatabase::open(&path)
        .err()
        .expect("open should reject non-Mundis db");

    assert!(error.to_string().contains("not a Mundis save database"));
}
