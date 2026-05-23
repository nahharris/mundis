use std::hint::black_box;

use criterion::{BatchSize, BenchmarkId, Criterion, criterion_group, criterion_main};
use mundis_core::{
    config::{SimulationConfig, WorldSize},
    export::{render_json, render_markdown, render_text},
    simulation::{Simulation, SimulationSeed},
    storage::SaveDatabase,
    world::World,
};

fn bench_generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("generation");

    for regions in [6, 24, 96] {
        group.bench_with_input(
            BenchmarkId::from_parameter(regions),
            &regions,
            |b, regions| {
                let config = SimulationConfig {
                    world: WorldSize { regions: *regions },
                    ..SimulationConfig::default()
                };

                b.iter(|| black_box(World::generate(&config, SimulationSeed::from_u64(42))));
            },
        );
    }

    group.finish();
}

fn bench_ticking(c: &mut Criterion) {
    c.bench_function("ticking/run_120_months", |b| {
        let config = SimulationConfig {
            months: 120,
            world: WorldSize { regions: 24 },
            ..SimulationConfig::default()
        };

        b.iter_batched(
            || Simulation::new(config.clone(), SimulationSeed::from_u64(42)),
            |mut simulation| black_box(simulation.run_months(config.months)),
            BatchSize::SmallInput,
        );
    });
}

fn bench_storage(c: &mut Criterion) {
    c.bench_function("storage/write_and_read_save", |b| {
        let config = SimulationConfig {
            months: 120,
            world: WorldSize { regions: 24 },
            ..SimulationConfig::default()
        };

        b.iter(|| {
            let temp_dir = tempfile::tempdir().expect("temp dir");
            let path = temp_dir.path().join("run.mundis.db");
            let mut simulation = Simulation::new(config.clone(), SimulationSeed::from_u64(42));
            let events = simulation.run_months(config.months);
            let snapshot = simulation.snapshot();

            let db = SaveDatabase::create(&path, &config, SimulationSeed::from_u64(42))
                .expect("create db");
            db.append_events(&events).expect("append events");
            db.store_snapshot(&snapshot).expect("store snapshot");
            drop(db);

            let reopened = SaveDatabase::open(&path).expect("open db");
            let loaded_events = reopened.load_events().expect("load events");
            let loaded_snapshot = reopened.load_latest_snapshot().expect("load snapshot");

            black_box((loaded_events, loaded_snapshot))
        });
    });
}

fn bench_export(c: &mut Criterion) {
    let config = SimulationConfig {
        months: 120,
        world: WorldSize { regions: 24 },
        ..SimulationConfig::default()
    };
    let mut simulation = Simulation::new(config.clone(), SimulationSeed::from_u64(42));
    let events = simulation.run_months(config.months);
    let mut group = c.benchmark_group("export");

    group.bench_function("text", |b| b.iter(|| black_box(render_text(&events))));
    group.bench_function("json", |b| {
        b.iter(|| black_box(render_json(&events).expect("JSON renders")))
    });
    group.bench_function("markdown", |b| {
        b.iter(|| black_box(render_markdown(&events)))
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_generation,
    bench_ticking,
    bench_storage,
    bench_export
);
criterion_main!(benches);
