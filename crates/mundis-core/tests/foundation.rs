use mundis_core::{
    civilization::{AllianceStatus, NamingTradition, PolityStatus, WarStatus},
    config::{
        CivilizationConfig, LivingHistoryConfig, SimulationBias, SimulationConfig, WorldSize,
    },
    export::{render_json, render_markdown, render_text},
    simulation::{
        EventSeverity, EventSubject, EventType, SettlementStatus, Simulation, SimulationEvent,
        SimulationSeed, SubsistenceMode,
    },
    storage::SaveDatabase,
    world::{Resource, World},
};
use mundis_core::{
    history::{HistoryQuery, SubjectFilter, atlas_state},
    scenario::ScenarioConfig,
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
fn scenario_simulation_overrides_base_config() {
    let input = r#"
        [simulation]
        months = 18
        bias = "harsh"

        [simulation.world]
        regions = 4

        [simulation.civilization]
        enabled = false

        [simulation.living_history]
        initial_settlements = 1
    "#;
    let scenario = ScenarioConfig::from_toml(input).expect("scenario parses");
    let base = SimulationConfig {
        months: 6,
        world: WorldSize { regions: 9 },
        bias: SimulationBias::Peaceful,
        ..SimulationConfig::default()
    };

    let compiled = scenario
        .compile(base, SimulationSeed::from_u64(10))
        .expect("scenario compiles");

    assert_eq!(compiled.config.months, 18);
    assert_eq!(compiled.config.world.regions, 4);
    assert_eq!(compiled.config.bias, SimulationBias::Harsh);
    assert!(!compiled.config.civilization.enabled);
    assert_eq!(compiled.config.living_history.initial_settlements, 1);
}

#[test]
fn partial_scenario_authors_initial_state_and_fills_missing_regions() {
    let input = r#"
        [simulation.world]
        regions = 3

        [[regions]]
        id = "coast"
        name = "Bright Coast"
        climate = "temperate"
        biome = "grassland"
        resources = ["fish", "salt"]
        carrying_capacity = 2500
        neighbors = ["hills"]

        [[regions]]
        id = "hills"
        name = "Copper Hills"
        climate = "arid"
        biome = "desert"
        resources = ["copper"]
        carrying_capacity = 900
        neighbors = ["coast"]

        [[cultures]]
        id = "mariners"
        name = "Mariners"
        origin_region = "coast"
        traits = ["maritime", "mercantile"]

        [[settlements]]
        id = "harbor"
        name = "First Harbor"
        region = "coast"
        stability = 88
        culture = "mariners"
        population = 720

        [[background_events]]
        id = "landing"
        summary = "The first ships landed on Bright Coast."
        tags = ["origin"]
        regions = ["coast"]
        settlements = ["harbor"]
        cultures = ["mariners"]
    "#;
    let scenario = ScenarioConfig::from_toml(input).expect("scenario parses");

    let compiled = scenario
        .compile(SimulationConfig::default(), SimulationSeed::from_u64(10))
        .expect("scenario compiles");
    let mut simulation = Simulation::from_compiled_scenario(compiled);

    let snapshot = simulation.snapshot();
    assert_eq!(snapshot.state.world.regions.len(), 3);
    assert_eq!(snapshot.state.world.regions[0].name, "Bright Coast");
    assert_eq!(snapshot.state.world.regions[1].name, "Copper Hills");
    assert_eq!(snapshot.state.settlements[0].name, "First Harbor");
    assert_eq!(snapshot.state.settlements[0].stability, 88);
    assert_eq!(snapshot.state.population_groups[0].population, 720);
    assert_eq!(snapshot.state.cultures[0].name, "Mariners");

    let events = simulation.run_months(1);
    assert_eq!(events[0].month, 0);
    assert_eq!(events[0].event_type, EventType::BackgroundEvent);
    assert_eq!(events[0].summary, "The first ships landed on Bright Coast.");
}

#[test]
fn scenario_rejects_unknown_references() {
    let input = r#"
        [[settlements]]
        id = "harbor"
        name = "First Harbor"
        region = "missing"
        population = 100
    "#;
    let scenario = ScenarioConfig::from_toml(input).expect("scenario parses");

    let error = scenario
        .compile(SimulationConfig::default(), SimulationSeed::from_u64(10))
        .expect_err("scenario should reject bad references")
        .to_string();

    assert!(error.contains("settlement 'harbor'"));
    assert!(error.contains("unknown region 'missing'"));
}

#[test]
fn scenario_rejects_authored_cultures_without_authored_population_state() {
    let input = r#"
        [[regions]]
        id = "coast"
        name = "Bright Coast"
        climate = "temperate"
        biome = "grassland"
        resources = ["fish"]
        carrying_capacity = 2500

        [[cultures]]
        id = "mariners"
        name = "Mariners"
        origin_region = "coast"
    "#;
    let scenario = ScenarioConfig::from_toml(input).expect("scenario parses");

    let error = scenario
        .compile(SimulationConfig::default(), SimulationSeed::from_u64(10))
        .expect_err("scenario should reject dangling authored cultures")
        .to_string();

    assert!(error.contains("authored cultures require authored settlements or population_groups"));
}

#[test]
fn save_database_preserves_scenario_sources() {
    let temp = tempfile::tempdir().expect("temp dir");
    let path = temp.path().join("scenario.mundis");
    let config = SimulationConfig::default();
    let base_config_toml = "months = 12\n";
    let scenario_toml = "[simulation]\nmonths = 3\n";

    let db = SaveDatabase::create_with_sources(
        &path,
        &config,
        SimulationSeed::from_u64(99),
        Some(base_config_toml),
        Some(scenario_toml),
    )
    .expect("create db");
    drop(db);

    let reopened = SaveDatabase::open(&path).expect("open db");
    assert_eq!(
        reopened
            .load_base_config_source()
            .expect("load base config source"),
        Some(base_config_toml.to_string())
    );
    assert_eq!(
        reopened
            .load_scenario_source()
            .expect("load scenario source"),
        Some(scenario_toml.to_string())
    );
}

#[test]
fn save_database_rejects_existing_save_paths() {
    let temp = tempfile::tempdir().expect("temp dir");
    let path = temp.path().join("existing.mundis");
    let config = SimulationConfig::default();

    SaveDatabase::create(&path, &config, SimulationSeed::from_u64(99)).expect("create db");
    let error = match SaveDatabase::create(&path, &config, SimulationSeed::from_u64(99)) {
        Ok(_) => panic!("existing save path should be rejected"),
        Err(error) => error.to_string(),
    };

    assert!(error.contains("save database already exists"));
}

#[test]
fn save_database_uses_clean_history_schema_version() {
    let temp = tempfile::tempdir().expect("temp dir");
    let path = temp.path().join("schema.mundis");
    let config = SimulationConfig::default();

    SaveDatabase::create(&path, &config, SimulationSeed::from_u64(99)).expect("create db");

    let connection = rusqlite::Connection::open(&path).expect("open sqlite db");
    let version: String = connection
        .query_row(
            "SELECT value FROM metadata WHERE key = 'schema_version'",
            [],
            |row| row.get(0),
        )
        .expect("schema version");
    assert_eq!(version, "1");
}

#[test]
fn old_save_schema_versions_are_rejected_without_migration() {
    let temp = tempfile::tempdir().expect("temp dir");
    let path = temp.path().join("old-schema.mundis");
    let connection = rusqlite::Connection::open(&path).expect("create sqlite db");
    connection
        .execute_batch(
            "
            CREATE TABLE metadata (key TEXT PRIMARY KEY, value TEXT NOT NULL);
            INSERT INTO metadata (key, value) VALUES ('schema_version', '4');
            ",
        )
        .expect("old schema metadata");
    drop(connection);

    let error = match SaveDatabase::open(&path) {
        Ok(_) => panic!("old schema should be rejected"),
        Err(error) => error.to_string(),
    };

    assert!(error.contains("unsupported save schema version 4"));
}

#[test]
fn save_database_indexes_and_queries_events() {
    let temp = tempfile::tempdir().expect("temp dir");
    let path = temp.path().join("history.mundis");
    let config = SimulationConfig {
        months: 18,
        ..arc_heavy_config()
    };
    let seed = SimulationSeed::from_u64(5);
    let mut simulation = Simulation::new(config.clone(), seed);
    let db = SaveDatabase::create(&path, &config, seed).expect("create db");
    db.store_snapshot(&simulation.snapshot())
        .expect("store month zero");

    for _ in 0..config.months {
        let events = simulation.tick_month();
        db.append_events(&events).expect("append events");
        db.store_snapshot(&simulation.snapshot())
            .expect("store monthly snapshot");
    }

    let polity_events = db
        .query_events(&HistoryQuery {
            from_month: Some(1),
            to_month: Some(config.months),
            tag: Some("polity".to_string()),
            event_type: None,
            severity: Some(EventSeverity::Important),
            subject: None,
        })
        .expect("query events");

    assert!(!polity_events.is_empty());
    assert!(polity_events.iter().all(|event| event.month >= 1));
    assert!(
        polity_events
            .iter()
            .all(|event| event.month <= config.months)
    );
    assert!(
        polity_events
            .iter()
            .all(|event| event.tags.iter().any(|tag| tag == "polity"))
    );
    assert!(
        polity_events
            .iter()
            .all(|event| event.severity == EventSeverity::Important)
    );
}

#[test]
fn save_database_loads_exact_state_at_month() {
    let temp = tempfile::tempdir().expect("temp dir");
    let path = temp.path().join("state.mundis");
    let config = SimulationConfig {
        months: 8,
        ..SimulationConfig::default()
    };
    let seed = SimulationSeed::from_u64(27);
    let mut simulation = Simulation::new(config.clone(), seed);
    let db = SaveDatabase::create(&path, &config, seed).expect("create db");
    db.store_snapshot(&simulation.snapshot())
        .expect("store month zero");

    let mut expected = None;
    for _ in 0..config.months {
        let events = simulation.tick_month();
        db.append_events(&events).expect("append events");
        let snapshot = simulation.snapshot();
        if snapshot.state.month == 5 {
            expected = Some(snapshot.clone());
        }
        db.store_snapshot(&snapshot)
            .expect("store monthly snapshot");
    }

    assert_eq!(
        db.load_snapshot_at_month(5).expect("load month 5"),
        expected.expect("expected month 5")
    );
}

#[test]
fn entity_history_filters_by_subject() {
    let temp = tempfile::tempdir().expect("temp dir");
    let path = temp.path().join("entity.mundis");
    let config = arc_heavy_config();
    let seed = SimulationSeed::from_u64(5);
    let mut simulation = Simulation::new(config.clone(), seed);
    let db = SaveDatabase::create(&path, &config, seed).expect("create db");

    for _ in 0..config.months {
        let events = simulation.tick_month();
        db.append_events(&events).expect("append events");
    }

    let events = db
        .entity_history(SubjectFilter::Polity(0))
        .expect("entity history");

    assert!(!events.is_empty());
    assert!(
        events
            .iter()
            .all(|event| event.subjects.contains(&EventSubject::Polity(0)))
    );
}

#[test]
fn atlas_state_projects_snapshot_for_ui() {
    let config = SimulationConfig {
        months: 3,
        ..SimulationConfig::default()
    };
    let mut simulation = Simulation::new(config.clone(), SimulationSeed::from_u64(42));
    simulation.run_months(config.months);

    let atlas = atlas_state(&simulation.snapshot());

    assert_eq!(atlas.month, config.months);
    assert_eq!(
        atlas.regions.len(),
        simulation.snapshot().state.world.regions.len()
    );
    assert_eq!(
        atlas.settlements.len(),
        simulation.snapshot().state.settlements.len()
    );
    assert_eq!(atlas.population, simulation.snapshot().state.population);
}

#[test]
fn atlas_state_exposes_region_population_and_settlement_status() {
    let config = SimulationConfig {
        months: 6,
        world: WorldSize { regions: 2 },
        living_history: LivingHistoryConfig {
            initial_settlements: 2,
            initial_population_per_mille: 3_000,
            monthly_growth_per_mille: 0,
            migration_pressure_threshold_per_mille: 1_050,
            decline_pressure_threshold_per_mille: 1_100,
            migrant_split_per_mille: 250,
        },
        ..SimulationConfig::default()
    };
    let mut simulation = Simulation::new(config.clone(), SimulationSeed::from_u64(5));
    simulation.run_months(config.months);

    let snapshot = simulation.snapshot();
    let atlas = atlas_state(&snapshot);

    assert_eq!(
        atlas
            .regions
            .iter()
            .map(|region| region.population)
            .sum::<u64>(),
        atlas.population
    );
    assert!(
        atlas
            .settlements
            .iter()
            .any(|settlement| settlement.status == SettlementStatus::Abandoned)
    );
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
fn subsistence_and_resources_shape_effective_capacity() {
    let config = SimulationConfig {
        world: WorldSize { regions: 8 },
        ..SimulationConfig::default()
    };
    let simulation = Simulation::new(config, SimulationSeed::from_u64(7));
    let snapshot = simulation.snapshot();
    let grain_region = snapshot
        .state
        .world
        .regions
        .iter()
        .find(|region| {
            region
                .resources
                .iter()
                .any(|resource| matches!(resource, Resource::Grain))
        })
        .expect("generated grain region");

    let farming_capacity = snapshot
        .state
        .effective_capacity(grain_region.id, SubsistenceMode::Farming);
    let foraging_capacity = snapshot
        .state
        .effective_capacity(grain_region.id, SubsistenceMode::Foraging);

    assert!(farming_capacity > foraging_capacity);
}

#[test]
fn environmental_stress_reduces_stability_and_emits_structured_event() {
    let config = SimulationConfig {
        months: 12,
        world: WorldSize { regions: 3 },
        living_history: LivingHistoryConfig {
            initial_settlements: 1,
            initial_population_per_mille: 700,
            monthly_growth_per_mille: 0,
            migration_pressure_threshold_per_mille: 3_000,
            decline_pressure_threshold_per_mille: 4_000,
            migrant_split_per_mille: 250,
        },
        ..SimulationConfig::default()
    };
    let mut simulation = Simulation::new(config.clone(), SimulationSeed::from_u64(3));
    let starting_stability = simulation.snapshot().state.settlements[0].stability;

    let events = simulation.run_months(config.months);
    let snapshot = simulation.snapshot();
    let stress = events
        .iter()
        .find(|event| event.event_type == EventType::EnvironmentalStress)
        .expect("environmental stress event");

    assert!(
        stress
            .causes
            .iter()
            .any(|cause| cause.contains("seasonal stress"))
    );
    assert!(
        stress
            .subjects
            .iter()
            .any(|subject| matches!(subject, EventSubject::Settlement(0)))
    );
    assert!(snapshot.state.settlements[0].stability < starting_stability);
}

#[test]
fn sustained_pressure_can_abandon_a_settlement_with_population_loss() {
    let config = SimulationConfig {
        months: 6,
        world: WorldSize { regions: 2 },
        living_history: LivingHistoryConfig {
            initial_settlements: 2,
            initial_population_per_mille: 3_000,
            monthly_growth_per_mille: 0,
            migration_pressure_threshold_per_mille: 1_050,
            decline_pressure_threshold_per_mille: 1_100,
            migrant_split_per_mille: 250,
        },
        ..SimulationConfig::default()
    };
    let mut simulation = Simulation::new(config.clone(), SimulationSeed::from_u64(5));
    let initial_population = simulation.snapshot().state.total_population();

    let events = simulation.run_months(config.months);
    let snapshot = simulation.snapshot();

    let abandonment = events
        .iter()
        .find(|event| event.event_type == EventType::SettlementAbandoned)
        .expect("settlement abandonment event");

    assert!(
        snapshot
            .state
            .settlements
            .iter()
            .any(|settlement| settlement.status == SettlementStatus::Abandoned)
    );
    assert!(snapshot.state.total_population() < initial_population);
    assert!(
        abandonment
            .consequences
            .iter()
            .any(|consequence| consequence.contains("population loss"))
    );
}

#[test]
fn abandoned_settlements_stop_growth_and_pressure_handling() {
    let config = SimulationConfig {
        months: 8,
        world: WorldSize { regions: 2 },
        living_history: LivingHistoryConfig {
            initial_settlements: 2,
            initial_population_per_mille: 3_000,
            monthly_growth_per_mille: 50,
            migration_pressure_threshold_per_mille: 1_050,
            decline_pressure_threshold_per_mille: 1_100,
            migrant_split_per_mille: 250,
        },
        ..SimulationConfig::default()
    };
    let mut simulation = Simulation::new(config.clone(), SimulationSeed::from_u64(5));

    let events = simulation.run_months(config.months);
    let abandonment = events
        .iter()
        .find(|event| event.event_type == EventType::SettlementAbandoned)
        .expect("settlement abandonment event");
    let abandoned_id = abandonment
        .subjects
        .iter()
        .find_map(|subject| match subject {
            EventSubject::Settlement(id) => Some(*id),
            _ => None,
        })
        .expect("abandoned settlement subject");

    assert!(
        events
            .iter()
            .filter(|event| event.month > abandonment.month)
            .all(|event| !event
                .subjects
                .contains(&EventSubject::Settlement(abandoned_id)))
    );
}

#[test]
fn emitted_event_subjects_reference_existing_state_entities() {
    let config = SimulationConfig {
        months: 12,
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

    for event in &events {
        for subject in &event.subjects {
            match subject {
                EventSubject::Region(id) => assert!(*id < snapshot.state.world.regions.len()),
                EventSubject::Settlement(id) => {
                    assert!(
                        snapshot
                            .state
                            .settlements
                            .iter()
                            .any(|settlement| settlement.id == *id)
                    )
                }
                EventSubject::PopulationGroup(id) => assert!(
                    snapshot
                        .state
                        .population_groups
                        .iter()
                        .any(|group| group.id == *id)
                ),
                EventSubject::Culture(id) => assert!(
                    snapshot
                        .state
                        .cultures
                        .iter()
                        .any(|culture| culture.id == *id)
                ),
                EventSubject::Polity(id) => assert!(
                    snapshot
                        .state
                        .polities
                        .iter()
                        .any(|polity| polity.id == *id)
                ),
            }
        }
    }
}

#[test]
fn old_toml_configs_parse_with_civilization_defaults() {
    let input = r#"
months = 24
bias = "plausible"

[world]
regions = 8

[living_history]
initial_settlements = 2
initial_population_per_mille = 250
monthly_growth_per_mille = 8
migration_pressure_threshold_per_mille = 1100
decline_pressure_threshold_per_mille = 1600
migrant_split_per_mille = 200

[output]
verbosity = "chronicle"
"#;

    let config = SimulationConfig::from_toml(input).expect("old config parses");

    assert_eq!(config.civilization, CivilizationConfig::default());
}

#[test]
fn simulation_initializes_cultures_for_population_groups() {
    let config = SimulationConfig {
        world: WorldSize { regions: 6 },
        living_history: LivingHistoryConfig {
            initial_settlements: 2,
            initial_population_per_mille: 400,
            ..LivingHistoryConfig::default()
        },
        ..SimulationConfig::default()
    };

    let first = Simulation::new(config.clone(), SimulationSeed::from_u64(42));
    let second = Simulation::new(config, SimulationSeed::from_u64(42));
    let snapshot = first.snapshot();

    assert_eq!(first.snapshot(), second.snapshot());
    assert_eq!(snapshot.state.cultures.len(), 2);
    assert!(
        snapshot
            .state
            .population_groups
            .iter()
            .all(|group| group.culture.is_some())
    );
    for culture in &snapshot.state.cultures {
        assert!(!culture.name.is_empty());
        assert!(!culture.traits.is_empty());
        assert!(culture.origin_region < snapshot.state.world.regions.len());
    }
}

#[test]
fn disabled_civilization_layer_leaves_identity_state_empty() {
    let config = SimulationConfig {
        civilization: CivilizationConfig {
            enabled: false,
            ..CivilizationConfig::default()
        },
        ..SimulationConfig::default()
    };
    let mut simulation = Simulation::new(config.clone(), SimulationSeed::from_u64(42));

    let events = simulation.run_months(config.months);
    let snapshot = simulation.snapshot();

    assert!(snapshot.state.cultures.is_empty());
    assert!(snapshot.state.polities.is_empty());
    assert!(
        snapshot
            .state
            .population_groups
            .iter()
            .all(|group| group.culture.is_none())
    );
    assert!(
        events
            .iter()
            .all(|event| event.event_type != EventType::PolityFounded)
    );
}

#[test]
fn seeded_runs_found_polities_with_valid_ownership() {
    let config = SimulationConfig {
        months: 1,
        world: WorldSize { regions: 5 },
        living_history: LivingHistoryConfig {
            initial_settlements: 2,
            initial_population_per_mille: 900,
            monthly_growth_per_mille: 0,
            ..LivingHistoryConfig::default()
        },
        civilization: CivilizationConfig {
            polity_foundation_population: 100,
            ..CivilizationConfig::default()
        },
        ..SimulationConfig::default()
    };
    let mut simulation = Simulation::new(config.clone(), SimulationSeed::from_u64(12));

    let events = simulation.run_months(config.months);
    let snapshot = simulation.snapshot();

    assert!(
        events
            .iter()
            .any(|event| event.event_type == EventType::PolityFounded)
    );
    assert!(!snapshot.state.polities.is_empty());
    for polity in &snapshot.state.polities {
        assert_eq!(polity.status, PolityStatus::Active);
        assert!(polity.primary_culture < snapshot.state.cultures.len());
        assert!(polity.capital < snapshot.state.settlements.len());
        assert!(!polity.controlled_settlements.is_empty());
        for settlement_id in &polity.controlled_settlements {
            assert_eq!(
                snapshot.state.settlements[*settlement_id].polity,
                Some(polity.id)
            );
        }
        for region_id in &polity.controlled_regions {
            assert!(*region_id < snapshot.state.world.regions.len());
        }
    }
}

#[test]
fn civilization_layer_forms_trade_links_between_neighboring_polities() {
    let config = SimulationConfig {
        months: 12,
        world: WorldSize { regions: 6 },
        living_history: LivingHistoryConfig {
            initial_settlements: 3,
            initial_population_per_mille: 900,
            monthly_growth_per_mille: 0,
            ..LivingHistoryConfig::default()
        },
        civilization: CivilizationConfig {
            polity_foundation_population: 100,
            trade_interval_months: 1,
            ..CivilizationConfig::default()
        },
        ..SimulationConfig::default()
    };
    let mut simulation = Simulation::new(config.clone(), SimulationSeed::from_u64(7));

    let events = simulation.run_months(config.months);
    let snapshot = simulation.snapshot();

    let trade = events
        .iter()
        .find(|event| event.event_type == EventType::TradeLinkFormed)
        .expect("trade link event");
    assert!(
        trade
            .subjects
            .iter()
            .filter(|subject| matches!(subject, EventSubject::Polity(_)))
            .count()
            >= 2
    );
    assert!(!snapshot.state.trade_links.is_empty());
    for link in &snapshot.state.trade_links {
        assert!(link.polities.0 < snapshot.state.polities.len());
        assert!(link.polities.1 < snapshot.state.polities.len());
    }
}

#[test]
fn border_tension_events_are_caused_by_neighboring_polity_pressure() {
    let config = SimulationConfig {
        months: 12,
        world: WorldSize { regions: 4 },
        living_history: LivingHistoryConfig {
            initial_settlements: 2,
            initial_population_per_mille: 1_600,
            monthly_growth_per_mille: 0,
            migration_pressure_threshold_per_mille: 3_000,
            decline_pressure_threshold_per_mille: 4_000,
            migrant_split_per_mille: 0,
        },
        civilization: CivilizationConfig {
            polity_foundation_population: 100,
            tension_interval_months: 1,
            ..CivilizationConfig::default()
        },
        ..SimulationConfig::default()
    };
    let mut simulation = Simulation::new(config.clone(), SimulationSeed::from_u64(5));

    let events = simulation.run_months(config.months);
    let tension = events
        .iter()
        .find(|event| event.event_type == EventType::BorderTension)
        .expect("border tension event");

    assert!(
        tension
            .causes
            .iter()
            .any(|cause| cause.contains("pressure") || cause.contains("border"))
    );
    assert!(
        tension
            .subjects
            .iter()
            .filter(|subject| matches!(subject, EventSubject::Polity(_)))
            .count()
            >= 2
    );
}

#[test]
fn cultures_drift_when_they_span_multiple_regions() {
    let config = SimulationConfig {
        months: 24,
        world: WorldSize { regions: 6 },
        living_history: LivingHistoryConfig {
            initial_settlements: 1,
            initial_population_per_mille: 1_700,
            monthly_growth_per_mille: 10,
            migration_pressure_threshold_per_mille: 1_050,
            decline_pressure_threshold_per_mille: 3_000,
            migrant_split_per_mille: 300,
        },
        civilization: CivilizationConfig {
            cultural_drift_interval_months: 1,
            polity_foundation_population: 100,
            ..CivilizationConfig::default()
        },
        ..SimulationConfig::default()
    };
    let mut simulation = Simulation::new(config.clone(), SimulationSeed::from_u64(9));

    let events = simulation.run_months(config.months);

    assert!(events.iter().any(|event| {
        event.event_type == EventType::CultureDrift
            && event
                .subjects
                .iter()
                .any(|subject| matches!(subject, EventSubject::Culture(_)))
    }));
}

#[test]
fn sustained_border_tension_can_collapse_a_polity_and_release_ownership() {
    let config = SimulationConfig {
        months: 36,
        world: WorldSize { regions: 4 },
        living_history: LivingHistoryConfig {
            initial_settlements: 2,
            initial_population_per_mille: 1_800,
            monthly_growth_per_mille: 0,
            migration_pressure_threshold_per_mille: 3_000,
            decline_pressure_threshold_per_mille: 4_000,
            migrant_split_per_mille: 0,
        },
        civilization: CivilizationConfig {
            polity_foundation_population: 100,
            tension_interval_months: 1,
            collapse_cohesion_threshold: 80,
            ..CivilizationConfig::default()
        },
        ..SimulationConfig::default()
    };
    let mut simulation = Simulation::new(config.clone(), SimulationSeed::from_u64(5));

    let events = simulation.run_months(config.months);
    let snapshot = simulation.snapshot();
    let collapse = events
        .iter()
        .find(|event| event.event_type == EventType::PolityCollapse)
        .expect("polity collapse event");
    assert_eq!(
        events
            .iter()
            .filter(|event| event.event_type == EventType::PolityFounded)
            .count(),
        2
    );
    let polity_id = collapse
        .subjects
        .iter()
        .find_map(|subject| match subject {
            EventSubject::Polity(id) => Some(*id),
            _ => None,
        })
        .expect("collapsed polity subject");

    assert_eq!(
        snapshot.state.polities[polity_id].status,
        PolityStatus::Collapsed
    );
    assert!(
        snapshot
            .state
            .settlements
            .iter()
            .all(|settlement| settlement.polity != Some(polity_id))
    );
}

#[test]
fn abandoned_settlements_leave_active_polity_regions_consistent() {
    let config = SimulationConfig {
        months: 6,
        world: WorldSize { regions: 2 },
        living_history: LivingHistoryConfig {
            initial_settlements: 2,
            initial_population_per_mille: 3_000,
            monthly_growth_per_mille: 0,
            migration_pressure_threshold_per_mille: 1_050,
            decline_pressure_threshold_per_mille: 1_100,
            migrant_split_per_mille: 250,
        },
        civilization: CivilizationConfig {
            polity_foundation_population: 100,
            collapse_cohesion_threshold: -100,
            ..CivilizationConfig::default()
        },
        ..SimulationConfig::default()
    };
    let mut simulation = Simulation::new(config.clone(), SimulationSeed::from_u64(5));

    let events = simulation.run_months(config.months);
    let snapshot = simulation.snapshot();

    assert!(
        events
            .iter()
            .any(|event| event.event_type == EventType::SettlementAbandoned)
    );
    for polity in snapshot
        .state
        .polities
        .iter()
        .filter(|polity| polity.status == PolityStatus::Active)
    {
        assert!(!polity.controlled_settlements.is_empty());
        let mut expected_regions = polity
            .controlled_settlements
            .iter()
            .map(|settlement_id| snapshot.state.settlements[*settlement_id].region)
            .collect::<Vec<_>>();
        expected_regions.sort_unstable();
        expected_regions.dedup();
        assert_eq!(polity.controlled_regions, expected_regions);
    }
}

#[test]
fn old_civilization_toml_parses_with_arc_defaults() {
    let input = r#"
months = 240
bias = "plausible"

[world]
regions = 8

[civilization]
enabled = true
polity_foundation_population = 500
expansion_pressure_threshold_per_mille = 900
trade_interval_months = 12
tension_interval_months = 12
cultural_drift_interval_months = 24
collapse_cohesion_threshold = 0

[living_history]
initial_settlements = 2
initial_population_per_mille = 250
monthly_growth_per_mille = 8
migration_pressure_threshold_per_mille = 1100
decline_pressure_threshold_per_mille = 1600
migrant_split_per_mille = 200

[output]
verbosity = "chronicle"
"#;

    let config = SimulationConfig::from_toml(input).expect("phase one config parses");

    assert_eq!(config.civilization.alliance_interval_months, 24);
    assert_eq!(config.civilization.war_interval_months, 12);
    assert_eq!(config.civilization.assimilation_interval_months, 24);
    assert_eq!(config.civilization.fragmentation_interval_months, 12);
    assert_eq!(config.civilization.succession_interval_months, 120);
    assert_eq!(config.civilization.war_tension_threshold, 60);
}

#[test]
fn cultures_have_naming_traditions_and_polities_have_institutions() {
    let config = SimulationConfig {
        months: 1,
        world: WorldSize { regions: 4 },
        living_history: LivingHistoryConfig {
            initial_settlements: 2,
            initial_population_per_mille: 900,
            monthly_growth_per_mille: 0,
            ..LivingHistoryConfig::default()
        },
        civilization: CivilizationConfig {
            polity_foundation_population: 100,
            ..CivilizationConfig::default()
        },
        ..SimulationConfig::default()
    };
    let mut simulation = Simulation::new(config.clone(), SimulationSeed::from_u64(12));

    simulation.run_months(config.months);
    let snapshot = simulation.snapshot();

    assert!(
        snapshot
            .state
            .cultures
            .iter()
            .all(|culture| matches!(culture.naming, NamingTradition::PrefixSuffix { .. }))
    );
    assert!(
        snapshot
            .state
            .polities
            .iter()
            .all(|polity| !polity.institutions.is_empty())
    );
}

#[test]
fn multi_century_civilization_run_explains_rise_conflict_treaty_and_collapse() {
    let config = arc_heavy_config();
    let mut simulation = Simulation::new(config.clone(), SimulationSeed::from_u64(5));

    let events = simulation.run_months(config.months);
    let snapshot = simulation.snapshot();

    let founded = first_event_month(&events, EventType::PolityFounded);
    let alliance = first_event_month(&events, EventType::AllianceFormed);
    let war = first_event_month(&events, EventType::WarDeclared);
    let treaty = first_event_month(&events, EventType::TreatySigned);
    let assimilation = first_event_month(&events, EventType::Assimilation);
    let fragmentation = first_event_month(&events, EventType::PolityFragmented);
    let collapse = first_event_month(&events, EventType::PolityCollapse);

    assert!(founded < war);
    assert!(alliance >= founded);
    assert!(war < treaty);
    assert!(assimilation >= founded);
    assert!(fragmentation >= founded);
    assert!(collapse >= war);
    assert!(!snapshot.state.alliances.is_empty());
    assert!(!snapshot.state.wars.is_empty());
    assert!(!snapshot.state.treaties.is_empty());
}

#[test]
fn wars_start_from_high_rivalry_and_end_with_treaties() {
    let config = arc_heavy_config();
    let mut simulation = Simulation::new(config.clone(), SimulationSeed::from_u64(5));

    let events = simulation.run_months(config.months);
    let snapshot = simulation.snapshot();
    let war = snapshot.state.wars.first().expect("war");

    assert_eq!(war.status, WarStatus::Ended);
    assert!(war.tension_at_start >= config.civilization.war_tension_threshold);
    assert!(war.ended_month.is_some());
    assert!(
        events
            .iter()
            .any(|event| event.event_type == EventType::WarEnded)
    );
    assert!(
        snapshot
            .state
            .treaties
            .iter()
            .any(|treaty| treaty.war == Some(war.id))
    );
}

#[test]
fn alliances_use_unique_active_polity_pairs() {
    let config = arc_heavy_config();
    let mut simulation = Simulation::new(config.clone(), SimulationSeed::from_u64(7));

    simulation.run_months(config.months);
    let snapshot = simulation.snapshot();
    let mut pairs = std::collections::HashSet::new();

    for alliance in &snapshot.state.alliances {
        assert!(alliance.polities.0 < alliance.polities.1);
        assert!(pairs.insert(alliance.polities));
        assert!(alliance.polities.0 < snapshot.state.polities.len());
        assert!(alliance.polities.1 < snapshot.state.polities.len());
        assert!(matches!(
            alliance.status,
            AllianceStatus::Active | AllianceStatus::Broken
        ));
    }
}

#[test]
fn assimilation_changes_culture_without_changing_total_population() {
    let config = arc_heavy_config();
    let mut simulation = Simulation::new(config.clone(), SimulationSeed::from_u64(9));
    let initial_population = simulation.snapshot().state.total_population();

    let events = simulation.run_months(config.months);
    let snapshot = simulation.snapshot();

    assert!(
        events
            .iter()
            .any(|event| event.event_type == EventType::Assimilation)
    );
    assert_eq!(snapshot.state.total_population(), initial_population);
}

#[test]
fn fragmentation_creates_child_polity_and_preserves_ownership() {
    let config = arc_heavy_config();
    let mut simulation = Simulation::new(config.clone(), SimulationSeed::from_u64(9));

    let events = simulation.run_months(config.months);
    let snapshot = simulation.snapshot();
    let fragment = events
        .iter()
        .find(|event| event.event_type == EventType::PolityFragmented)
        .expect("fragmentation event");
    let child_id = fragment
        .subjects
        .iter()
        .find_map(|subject| match subject {
            EventSubject::Polity(id) if snapshot.state.polities[*id].parent.is_some() => Some(*id),
            _ => None,
        })
        .expect("child polity subject");

    assert!(snapshot.state.polities[child_id].parent.is_some());
    if snapshot.state.polities[child_id].status == PolityStatus::Active {
        assert!(
            snapshot.state.polities[child_id]
                .controlled_settlements
                .iter()
                .all(
                    |settlement_id| snapshot.state.settlements[*settlement_id].polity
                        == Some(child_id)
                )
        );
    } else {
        assert!(
            snapshot
                .state
                .settlements
                .iter()
                .all(|settlement| settlement.polity != Some(child_id))
        );
    }
}

#[test]
fn succession_events_increment_polity_succession_count() {
    let config = arc_heavy_config();
    let mut simulation = Simulation::new(config.clone(), SimulationSeed::from_u64(12));

    let events = simulation.run_months(config.months);
    let snapshot = simulation.snapshot();

    assert!(
        events
            .iter()
            .any(|event| event.event_type == EventType::Succession)
    );
    assert!(
        snapshot
            .state
            .polities
            .iter()
            .any(|polity| polity.succession_count > 0)
    );
}

#[test]
fn active_wars_do_not_reference_collapsed_polities() {
    let config = arc_heavy_config();
    let mut simulation = Simulation::new(config.clone(), SimulationSeed::from_u64(5));

    simulation.run_months(config.months);
    let snapshot = simulation.snapshot();

    for war in snapshot
        .state
        .wars
        .iter()
        .filter(|war| war.status == WarStatus::Active)
    {
        assert_eq!(
            snapshot.state.polities[war.polities.0].status,
            PolityStatus::Active
        );
        assert_eq!(
            snapshot.state.polities[war.polities.1].status,
            PolityStatus::Active
        );
    }
}

#[test]
fn wars_ended_by_collapse_emit_closing_events() {
    let config = arc_heavy_config();
    let mut simulation = Simulation::new(config.clone(), SimulationSeed::from_u64(5));

    let events = simulation.run_months(config.months);
    let snapshot = simulation.snapshot();

    for war in snapshot
        .state
        .wars
        .iter()
        .filter(|war| war.status == WarStatus::Ended && war.ended_month.is_some())
    {
        assert!(events.iter().any(|event| {
            event.event_type == EventType::WarEnded
                && event.month == war.ended_month.expect("ended month")
                && event
                    .subjects
                    .contains(&EventSubject::Polity(war.polities.0))
                && event
                    .subjects
                    .contains(&EventSubject::Polity(war.polities.1))
        }));
    }
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

fn arc_heavy_config() -> SimulationConfig {
    SimulationConfig {
        months: 240,
        world: WorldSize { regions: 8 },
        living_history: LivingHistoryConfig {
            initial_settlements: 4,
            initial_population_per_mille: 1_600,
            monthly_growth_per_mille: 0,
            migration_pressure_threshold_per_mille: 3_000,
            decline_pressure_threshold_per_mille: 4_000,
            migrant_split_per_mille: 0,
        },
        civilization: CivilizationConfig {
            polity_foundation_population: 100,
            trade_interval_months: 1,
            tension_interval_months: 1,
            cultural_drift_interval_months: 1,
            alliance_interval_months: 1,
            war_interval_months: 1,
            assimilation_interval_months: 1,
            fragmentation_interval_months: 1,
            succession_interval_months: 12,
            war_tension_threshold: 30,
            fragmentation_cohesion_threshold: 85,
            war_end_score_threshold: 30,
            collapse_cohesion_threshold: -20,
            ..CivilizationConfig::default()
        },
        ..SimulationConfig::default()
    }
}

fn first_event_month(events: &[SimulationEvent], event_type: EventType) -> u32 {
    events
        .iter()
        .find(|event| event.event_type == event_type)
        .map(|event| event.month)
        .unwrap_or_else(|| panic!("missing event {event_type:?}"))
}
