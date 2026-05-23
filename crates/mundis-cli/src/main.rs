use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};
use clap::{Parser, Subcommand};
use mundis_core::{
    config::SimulationConfig,
    export::{render_json, render_markdown, render_text},
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
            months,
            save,
            export,
        } => run(seed, config, months, save, export),
        Command::Inspect { target } => match target {
            InspectCommand::World { seed, config } => inspect_world(seed, config),
        },
        Command::Replay { save, export } => replay(save, export),
    }
}

fn run(
    seed: u64,
    config_path: Option<PathBuf>,
    months: Option<u32>,
    save_path: Option<PathBuf>,
    export: ExportFormat,
) -> Result<()> {
    let mut config = load_config(config_path)?;
    if let Some(months) = months {
        config.months = months;
    }

    let seed = SimulationSeed::from_u64(seed);
    let mut simulation = Simulation::new(config.clone(), seed);
    let events = simulation.run_months(config.months);

    if let Some(path) = save_path {
        let db = SaveDatabase::create(&path, &config, seed).map_err(|error| {
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
    let config = load_config(config_path)?;
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

fn load_config(path: Option<PathBuf>) -> Result<SimulationConfig> {
    let Some(path) = path else {
        return Ok(SimulationConfig::default());
    };

    let input = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read config at {}", path.display()))?;
    SimulationConfig::from_toml(&input)
        .with_context(|| format!("failed to parse TOML config at {}", path.display()))
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
