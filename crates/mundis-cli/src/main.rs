use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};
use clap::{Parser, Subcommand};
use mundis_core::{
    config::SimulationConfig,
    export::{render_json, render_markdown, render_text},
    scenario::ScenarioConfig,
    simulation::{Simulation, SimulationSeed},
    storage::SaveDatabase,
    world::World,
};

#[derive(Debug, Parser)]
#[command(name = "mundis")]
#[command(about = "Text-first civilization simulation engine")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Run {
        #[arg(long, default_value_t = 1)]
        seed: u64,
        #[arg(long)]
        config: Option<PathBuf>,
        #[arg(long)]
        scenario: Option<PathBuf>,
        #[arg(long)]
        months: Option<u32>,
        #[arg(long)]
        save: Option<PathBuf>,
        #[arg(long, value_enum, default_value_t = ExportFormat::Text)]
        export: ExportFormat,
    },
    Inspect {
        #[command(subcommand)]
        target: InspectCommand,
    },
    Replay {
        save: PathBuf,
        #[arg(long, value_enum, default_value_t = ExportFormat::Text)]
        export: ExportFormat,
    },
}

#[derive(Clone, Debug, clap::ValueEnum)]
enum ExportFormat {
    Text,
    Markdown,
    Json,
}

#[derive(Debug, Subcommand)]
enum InspectCommand {
    World {
        #[arg(long, default_value_t = 1)]
        seed: u64,
        #[arg(long)]
        config: Option<PathBuf>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Run {
            seed,
            config,
            scenario,
            months,
            save,
            export,
        } => run(seed, config, scenario, months, save, export),
        Command::Inspect { target } => match target {
            InspectCommand::World { seed, config } => inspect_world(seed, config),
        },
        Command::Replay { save, export } => replay(save, export),
    }
}

fn run(
    seed: u64,
    config_path: Option<PathBuf>,
    scenario_path: Option<PathBuf>,
    months: Option<u32>,
    save_path: Option<PathBuf>,
    export: ExportFormat,
) -> Result<()> {
    let (mut base_config, base_config_source) = load_config(config_path)?;

    if let Some(path) = scenario_path {
        let scenario_source = std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read scenario at {}", path.display()))?;
        let scenario = ScenarioConfig::from_toml(&scenario_source)
            .with_context(|| format!("failed to parse TOML scenario at {}", path.display()))?;
        let seed = SimulationSeed::from_u64(seed);
        let compiled = scenario
            .compile(base_config, seed)
            .with_context(|| format!("failed to compile scenario at {}", path.display()))?;
        let mut config = compiled.config.clone();
        if let Some(months) = months {
            config.months = months;
        }
        let mut compiled = compiled;
        compiled.config = config;
        let mut simulation = Simulation::from_compiled_scenario(compiled);
        let events = simulation.run_months(simulation.config().months);

        if let Some(path) = save_path {
            let db = SaveDatabase::create_with_sources(
                &path,
                simulation.config(),
                seed,
                base_config_source.as_deref(),
                Some(&scenario_source),
            )
            .map_err(|error| {
                anyhow!(
                    "failed to create save database at {}: {error}",
                    path.display()
                )
            })?;
            db.append_events(&events)
                .map_err(|error| anyhow!("failed to append events: {error}"))?;
            db.store_snapshot(&simulation.snapshot())
                .map_err(|error| anyhow!("failed to store snapshot: {error}"))?;
        }

        print!("{}", render_events(&events, export)?);
        return Ok(());
    }

    if let Some(months) = months {
        base_config.months = months;
    }
    run_from_config(
        base_config,
        seed,
        save_path,
        export,
        base_config_source,
        None,
    )
}

fn run_from_config(
    config: SimulationConfig,
    seed: u64,
    save_path: Option<PathBuf>,
    export: ExportFormat,
    base_config_source: Option<String>,
    scenario_source: Option<String>,
) -> Result<()> {
    let seed = SimulationSeed::from_u64(seed);
    let mut simulation = Simulation::new(config.clone(), seed);
    let events = simulation.run_months(config.months);

    if let Some(path) = save_path {
        let db = SaveDatabase::create_with_sources(
            &path,
            &config,
            seed,
            base_config_source.as_deref(),
            scenario_source.as_deref(),
        )
        .map_err(|error| {
            anyhow!(
                "failed to create save database at {}: {error}",
                path.display()
            )
        })?;
        db.append_events(&events)
            .map_err(|error| anyhow!("failed to append events: {error}"))?;
        db.store_snapshot(&simulation.snapshot())
            .map_err(|error| anyhow!("failed to store snapshot: {error}"))?;
    }

    print!("{}", render_events(&events, export)?);
    Ok(())
}

fn inspect_world(seed: u64, config_path: Option<PathBuf>) -> Result<()> {
    let (config, _) = load_config(config_path)?;
    let world = World::generate(&config, SimulationSeed::from_u64(seed));

    println!("Mundis world: {} regions", world.regions.len());
    for region in world.regions {
        println!(
            "- {}: {:?} {:?}, capacity {}, neighbors {:?}, resources {:?}",
            region.name,
            region.climate,
            region.biome,
            region.carrying_capacity,
            region.neighbors,
            region.resources
        );
    }

    Ok(())
}

fn replay(save_path: PathBuf, export: ExportFormat) -> Result<()> {
    let db = SaveDatabase::open(&save_path).map_err(|error| {
        anyhow!(
            "failed to open save database at {}: {error}",
            save_path.display()
        )
    })?;
    let events = db
        .load_events()
        .map_err(|error| anyhow!("failed to load events: {error}"))?;

    print!("{}", render_events(&events, export)?);
    Ok(())
}

fn load_config(path: Option<PathBuf>) -> Result<(SimulationConfig, Option<String>)> {
    let Some(path) = path else {
        return Ok((SimulationConfig::default(), None));
    };

    let input = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read config at {}", path.display()))?;
    let config = SimulationConfig::from_toml(&input)
        .with_context(|| format!("failed to parse TOML config at {}", path.display()))?;
    Ok((config, Some(input)))
}

fn render_events(
    events: &[mundis_core::simulation::SimulationEvent],
    format: ExportFormat,
) -> Result<String> {
    match format {
        ExportFormat::Text => Ok(render_text(events)),
        ExportFormat::Markdown => Ok(render_markdown(events)),
        ExportFormat::Json => Ok(render_json(events)?),
    }
}
