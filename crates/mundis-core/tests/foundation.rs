use mundis_core::{
    config::{LivingHistoryConfig, SimulationBias, SimulationConfig, WorldSize},
    export::{render_json, render_markdown, render_text},
    simulation::{
        EventSeverity, EventSubject, EventType, SettlementStatus, Simulation, SimulationEvent,
        SimulationSeed,
    },
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

    assert!(events.len() >= config.months as usize);
    for (index, event) in events.iter().enumerate() {
        assert_eq!(event.id, index as u64 + 1);
        assert!((1..=config.months).contains(&event.month));
        assert!(!event.tags.is_empty());
        assert!(!event.summary.is_empty());
    }
    assert_eq!(snapshot.state.month, config.months);
    assert!(snapshot.state.world.is_connected());
    assert_eq!(snapshot.state.event_count, events.len() as u64);
}

#[test]
fn generated_world_region_names_are_unique_for_readable_chronicles() {
    let config = SimulationConfig {
        world: WorldSize { regions: 4 },
        ..SimulationConfig::default()
    };

    let world = World::generate(&config, SimulationSeed::from_u64(9));
    let names = world
        .regions
        .iter()
        .map(|region| region.name.as_str())
        .collect::<std::collections::HashSet<_>>();

    assert_eq!(names.len(), world.regions.len());
}

#[test]
fn simulation_initializes_deterministic_settlements_and_population_groups() {
    let config = SimulationConfig {
        world: WorldSize { regions: 6 },
        living_history: LivingHistoryConfig {
            initial_settlements: 2,
            initial_population_per_mille: 250,
            ..LivingHistoryConfig::default()
        },
        ..SimulationConfig::default()
    };
    let first = Simulation::new(config.clone(), SimulationSeed::from_u64(42));
    let second = Simulation::new(config, SimulationSeed::from_u64(42));

    let snapshot = first.snapshot();

    assert_eq!(snapshot, second.snapshot());
    assert_eq!(snapshot.state.settlements.len(), 2);
    assert_eq!(snapshot.state.population_groups.len(), 2);
    assert_eq!(
        snapshot.state.total_population(),
        snapshot
            .state
            .population_groups
            .iter()
            .map(|group| group.population)
            .sum::<u64>()
    );
    for settlement in &snapshot.state.settlements {
        assert_eq!(settlement.status, SettlementStatus::Active);
        assert!(
            snapshot
                .state
                .world
                .regions
                .iter()
                .any(|region| region.id == settlement.region)
        );
    }
}

#[test]
fn food_pressure_can_trigger_migration_and_found_a_neighbor_settlement() {
    let config = SimulationConfig {
        months: 3,
        world: WorldSize { regions: 4 },
        living_history: LivingHistoryConfig {
            initial_settlements: 1,
            initial_population_per_mille: 1_600,
            monthly_growth_per_mille: 25,
            migration_pressure_threshold_per_mille: 1_050,
            decline_pressure_threshold_per_mille: 3_000,
            migrant_split_per_mille: 250,
        },
        ..SimulationConfig::default()
    };
    let mut simulation = Simulation::new(config.clone(), SimulationSeed::from_u64(9));

    let events = simulation.run_months(config.months);
    let snapshot = simulation.snapshot();

    assert!(
        events
            .iter()
            .any(|event| event.event_type == EventType::FoodPressure)
    );
    let migration = events
        .iter()
        .find(|event| event.event_type == EventType::Migration)
        .expect("migration event");
    let founded = events
        .iter()
        .find(|event| event.event_type == EventType::SettlementFounded)
        .expect("settlement founding event");

    assert!(
        migration
            .causes
            .iter()
            .any(|cause| cause.contains("food pressure"))
    );
    assert!(
        migration
            .subjects
            .iter()
            .any(|subject| matches!(subject, EventSubject::PopulationGroup(_)))
    );
    assert!(
        founded
            .subjects
            .iter()
            .any(|subject| matches!(subject, EventSubject::Settlement(_)))
    );
    assert!(snapshot.state.settlements.len() > 1);
    assert_eq!(
        snapshot.state.total_population(),
        snapshot
            .state
            .population_groups
            .iter()
            .map(|group| group.population)
            .sum::<u64>()
    );
}

#[test]
fn zero_migrant_split_prevents_migration() {
    let config = SimulationConfig {
        months: 2,
        world: WorldSize { regions: 4 },
        living_history: LivingHistoryConfig {
            initial_settlements: 1,
            initial_population_per_mille: 1_600,
            monthly_growth_per_mille: 0,
            migration_pressure_threshold_per_mille: 1_050,
            decline_pressure_threshold_per_mille: 3_000,
            migrant_split_per_mille: 0,
        },
        ..SimulationConfig::default()
    };
    let mut simulation = Simulation::new(config.clone(), SimulationSeed::from_u64(9));

    let events = simulation.run_months(config.months);
    let snapshot = simulation.snapshot();

    assert!(
        events
            .iter()
            .any(|event| event.event_type == EventType::FoodPressure)
    );
    assert!(
        events
            .iter()
            .all(|event| event.event_type != EventType::Migration)
    );
    assert_eq!(snapshot.state.settlements.len(), 1);
}

#[test]
fn migrant_split_above_total_population_is_clamped() {
    let config = SimulationConfig {
        months: 1,
        world: WorldSize { regions: 4 },
        living_history: LivingHistoryConfig {
            initial_settlements: 1,
            initial_population_per_mille: 1_600,
            monthly_growth_per_mille: 0,
            migration_pressure_threshold_per_mille: 1_050,
            decline_pressure_threshold_per_mille: 3_000,
            migrant_split_per_mille: 2_000,
        },
        ..SimulationConfig::default()
    };
    let mut simulation = Simulation::new(config.clone(), SimulationSeed::from_u64(9));
    let initial_population = simulation.snapshot().state.total_population();

    let events = simulation.run_months(config.months);
    let snapshot = simulation.snapshot();

    assert!(
        events
            .iter()
            .any(|event| event.event_type == EventType::Migration)
    );
    assert_eq!(snapshot.state.total_population(), initial_population);
    assert!(
        snapshot
            .state
            .population_groups
            .iter()
            .all(|group| group.population <= initial_population)
    );
}

#[test]
fn trapped_food_pressure_can_decline_a_settlement() {
    let config = SimulationConfig {
        months: 2,
        world: WorldSize { regions: 2 },
        living_history: LivingHistoryConfig {
            initial_settlements: 2,
            initial_population_per_mille: 2_400,
            monthly_growth_per_mille: 0,
            migration_pressure_threshold_per_mille: 1_050,
            decline_pressure_threshold_per_mille: 1_250,
            migrant_split_per_mille: 250,
        },
        ..SimulationConfig::default()
    };
    let mut simulation = Simulation::new(config.clone(), SimulationSeed::from_u64(5));

    let events = simulation.run_months(config.months);
    let snapshot = simulation.snapshot();

    let decline = events
        .iter()
        .find(|event| event.event_type == EventType::SettlementDecline)
        .expect("settlement decline event");

    assert!(
        decline
            .causes
            .iter()
            .any(|cause| cause.contains("no open neighboring region"))
    );
    assert!(
        snapshot
            .state
            .settlements
            .iter()
            .any(|settlement| settlement.status == SettlementStatus::Declining)
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
fn text_json_and_markdown_exports_are_stable_renderers() {
    let events = vec![
        SimulationEvent {
            id: 1,
            month: 1,
            event_type: EventType::SettlementGrowth,
            severity: EventSeverity::Note,
            tags: vec!["population".to_string()],
            subjects: vec![EventSubject::Region(0)],
            causes: vec!["seeded test".to_string()],
            consequences: vec!["terraces filled".to_string()],
            summary: "Aruven stirred as terraces filled.".to_string(),
        },
        SimulationEvent {
            id: 2,
            month: 12,
            event_type: EventType::FoodPressure,
            severity: EventSeverity::Important,
            tags: vec!["region".to_string(), "politics".to_string()],
            subjects: vec![EventSubject::Region(1)],
            causes: vec!["border pressure".to_string()],
            consequences: vec!["customs changed".to_string()],
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
        "[\n  {\n    \"id\": 1,\n    \"month\": 1,\n    \"event_type\": \"settlement-growth\",\n    \"severity\": \"note\",\n    \"tags\": [\n      \"population\"\n    ],\n    \"subjects\": [\n      {\n        \"region\": 0\n      }\n    ],\n    \"causes\": [\n      \"seeded test\"\n    ],\n    \"consequences\": [\n      \"terraces filled\"\n    ],\n    \"summary\": \"Aruven stirred as terraces filled.\"\n  },\n  {\n    \"id\": 2,\n    \"month\": 12,\n    \"event_type\": \"food-pressure\",\n    \"severity\": \"important\",\n    \"tags\": [\n      \"region\",\n      \"politics\"\n    ],\n    \"subjects\": [\n      {\n        \"region\": 1\n      }\n    ],\n    \"causes\": [\n      \"border pressure\"\n    ],\n    \"consequences\": [\n      \"customs changed\"\n    ],\n    \"summary\": \"Beltor reshaped its border customs.\"\n  }\n]"
    );
}

#[test]
fn markdown_handles_zero_month_events_without_underflowing() {
    let events = vec![SimulationEvent {
        id: 1,
        month: 0,
        event_type: EventType::SettlementGrowth,
        severity: EventSeverity::Note,
        tags: vec!["malformed".to_string()],
        subjects: vec![],
        causes: vec![],
        consequences: vec![],
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
