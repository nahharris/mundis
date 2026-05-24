use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use mundis_app::{
    CreateSimulationRequest, ExportFormat as AppExportFormat, QueryEventsRequest,
    create_simulation, entity_history, export_events, export_snapshot, get_state_at_month,
    load_events, parse_event_type, parse_severity, parse_subject, query_events,
};
use mundis_core::{
    config::SimulationConfig, history::HistoryQuery, scenario::ScenarioConfig,
    simulation::SimulationSeed, world::World,
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

#[derive(Clone, Copy, Debug, clap::ValueEnum)]
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
    Events {
        #[arg(long)]
        save: PathBuf,
        #[arg(long)]
        from: Option<u32>,
        #[arg(long)]
        to: Option<u32>,
        #[arg(long)]
        tag: Option<String>,
        #[arg(long)]
        subject: Option<String>,
        #[arg(long)]
        event_type: Option<String>,
        #[arg(long)]
        severity: Option<String>,
        #[arg(long, value_enum, default_value_t = ExportFormat::Text)]
        export: ExportFormat,
    },
    State {
        #[arg(long)]
        save: PathBuf,
        #[arg(long)]
        month: u32,
        #[arg(long, value_enum, default_value_t = ExportFormat::Text)]
        export: ExportFormat,
    },
    Entity {
        #[arg(long)]
        save: PathBuf,
        #[arg(long)]
        subject: String,
        #[arg(long, value_enum, default_value_t = ExportFormat::Text)]
        export: ExportFormat,
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
            InspectCommand::Events {
                save,
                from,
                to,
                tag,
                subject,
                event_type,
                severity,
                export,
            } => inspect_events(InspectEventsArgs {
                save_path: save,
                from_month: from,
                to_month: to,
                tag,
                subject,
                event_type,
                severity,
                export,
            }),
            InspectCommand::State {
                save,
                month,
                export,
            } => inspect_state(save, month, export),
            InspectCommand::Entity {
                save,
                subject,
                export,
            } => inspect_entity(save, subject, export),
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
    let (base_config, base_config_source) = load_config(config_path)?;
    let (mut config, scenario_toml) = load_effective_config(base_config, scenario_path, seed)?;
    if let Some(months) = months {
        config.months = months;
    }

    let temp_dir = tempfile::tempdir()?;
    let save_path = save_path.unwrap_or_else(|| temp_dir.path().join("transient.mundis"));
    let summary = create_simulation(CreateSimulationRequest {
        seed,
        config,
        base_config_toml: base_config_source,
        scenario_toml,
        save_path,
    })?;

    print!("{}", export_events(&summary.events, export.into())?);
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
    let events = load_events(&save_path)?;
    print!("{}", export_events(&events, export.into())?);
    Ok(())
}

struct InspectEventsArgs {
    save_path: PathBuf,
    from_month: Option<u32>,
    to_month: Option<u32>,
    tag: Option<String>,
    subject: Option<String>,
    event_type: Option<String>,
    severity: Option<String>,
    export: ExportFormat,
}

fn inspect_events(args: InspectEventsArgs) -> Result<()> {
    let events = query_events(QueryEventsRequest {
        save_path: args.save_path,
        query: HistoryQuery {
            from_month: args.from_month,
            to_month: args.to_month,
            tag: args.tag,
            subject: args
                .subject
                .as_deref()
                .map(parse_subject)
                .transpose()
                .with_context(|| "failed to parse --subject")?,
            event_type: args
                .event_type
                .as_deref()
                .map(parse_event_type)
                .transpose()
                .with_context(|| "failed to parse --event-type")?,
            severity: args
                .severity
                .as_deref()
                .map(parse_severity)
                .transpose()
                .with_context(|| "failed to parse --severity")?,
        },
    })?;
    print!("{}", export_events(&events, args.export.into())?);
    Ok(())
}

fn inspect_state(save_path: PathBuf, month: u32, export: ExportFormat) -> Result<()> {
    let snapshot = get_state_at_month(&save_path, month)
        .with_context(|| format!("failed to load state at month {month}"))?;
    print!("{}", export_snapshot(&snapshot, export.into())?);
    Ok(())
}

fn inspect_entity(save_path: PathBuf, subject: String, export: ExportFormat) -> Result<()> {
    let subject = parse_subject(&subject).with_context(|| "failed to parse --subject")?;
    let events = entity_history(&save_path, subject)?;
    print!("{}", export_events(&events, export.into())?);
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

fn load_effective_config(
    base_config: SimulationConfig,
    scenario_path: Option<PathBuf>,
    seed: u64,
) -> Result<(SimulationConfig, Option<String>)> {
    let Some(path) = scenario_path else {
        return Ok((base_config, None));
    };
    let scenario_toml = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read scenario at {}", path.display()))?;
    let scenario = ScenarioConfig::from_toml(&scenario_toml)
        .with_context(|| format!("failed to parse TOML scenario at {}", path.display()))?;
    let compiled = scenario
        .compile(base_config, SimulationSeed::from_u64(seed))
        .with_context(|| format!("failed to compile scenario at {}", path.display()))?;
    Ok((compiled.config, Some(scenario_toml)))
}

impl From<ExportFormat> for AppExportFormat {
    fn from(value: ExportFormat) -> Self {
        match value {
            ExportFormat::Text => Self::Text,
            ExportFormat::Markdown => Self::Markdown,
            ExportFormat::Json => Self::Json,
        }
    }
}
