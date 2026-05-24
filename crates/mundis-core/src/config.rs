use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SimulationConfig {
    pub months: u32,
    pub world: WorldSize,
    #[serde(default)]
    pub civilization: CivilizationConfig,
    pub living_history: LivingHistoryConfig,
    #[serde(default)]
    pub history: HistoryConfig,
    pub bias: SimulationBias,
    pub output: OutputConfig,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryConfig {
    #[serde(default = "default_snapshot_interval_months")]
    pub snapshot_interval_months: u32,
}

fn default_snapshot_interval_months() -> u32 {
    6
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldSize {
    pub regions: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LivingHistoryConfig {
    pub initial_settlements: usize,
    pub initial_population_per_mille: u32,
    pub monthly_growth_per_mille: u32,
    pub migration_pressure_threshold_per_mille: u32,
    pub decline_pressure_threshold_per_mille: u32,
    pub migrant_split_per_mille: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CivilizationConfig {
    pub enabled: bool,
    pub polity_foundation_population: u64,
    pub expansion_pressure_threshold_per_mille: u32,
    pub trade_interval_months: u32,
    pub tension_interval_months: u32,
    pub cultural_drift_interval_months: u32,
    pub collapse_cohesion_threshold: i32,
    #[serde(default = "default_alliance_interval_months")]
    pub alliance_interval_months: u32,
    #[serde(default = "default_war_interval_months")]
    pub war_interval_months: u32,
    #[serde(default = "default_assimilation_interval_months")]
    pub assimilation_interval_months: u32,
    #[serde(default = "default_fragmentation_interval_months")]
    pub fragmentation_interval_months: u32,
    #[serde(default = "default_succession_interval_months")]
    pub succession_interval_months: u32,
    #[serde(default = "default_war_tension_threshold")]
    pub war_tension_threshold: i32,
    #[serde(default = "default_fragmentation_cohesion_threshold")]
    pub fragmentation_cohesion_threshold: i32,
    #[serde(default = "default_war_end_score_threshold")]
    pub war_end_score_threshold: i32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SimulationBias {
    Plausible,
    Dramatic,
    Harsh,
    Peaceful,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputConfig {
    pub verbosity: OutputVerbosity,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OutputVerbosity {
    Concise,
    Chronicle,
}

impl Default for HistoryConfig {
    fn default() -> Self {
        Self {
            snapshot_interval_months: default_snapshot_interval_months(),
        }
    }
}

impl Default for SimulationConfig {
    fn default() -> Self {
        Self {
            months: 120,
            world: WorldSize { regions: 6 },
            civilization: CivilizationConfig::default(),
            living_history: LivingHistoryConfig::default(),
            history: HistoryConfig::default(),
            bias: SimulationBias::Plausible,
            output: OutputConfig {
                verbosity: OutputVerbosity::Chronicle,
            },
        }
    }
}

impl Default for CivilizationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            polity_foundation_population: 500,
            expansion_pressure_threshold_per_mille: 900,
            trade_interval_months: 12,
            tension_interval_months: 12,
            cultural_drift_interval_months: 24,
            collapse_cohesion_threshold: 0,
            alliance_interval_months: default_alliance_interval_months(),
            war_interval_months: default_war_interval_months(),
            assimilation_interval_months: default_assimilation_interval_months(),
            fragmentation_interval_months: default_fragmentation_interval_months(),
            succession_interval_months: default_succession_interval_months(),
            war_tension_threshold: default_war_tension_threshold(),
            fragmentation_cohesion_threshold: default_fragmentation_cohesion_threshold(),
            war_end_score_threshold: default_war_end_score_threshold(),
        }
    }
}

fn default_alliance_interval_months() -> u32 {
    24
}

fn default_war_interval_months() -> u32 {
    12
}

fn default_assimilation_interval_months() -> u32 {
    24
}

fn default_fragmentation_interval_months() -> u32 {
    12
}

fn default_succession_interval_months() -> u32 {
    120
}

fn default_war_tension_threshold() -> i32 {
    60
}

fn default_fragmentation_cohesion_threshold() -> i32 {
    35
}

fn default_war_end_score_threshold() -> i32 {
    100
}

impl Default for LivingHistoryConfig {
    fn default() -> Self {
        Self {
            initial_settlements: 2,
            initial_population_per_mille: 250,
            monthly_growth_per_mille: 8,
            migration_pressure_threshold_per_mille: 1_100,
            decline_pressure_threshold_per_mille: 1_600,
            migrant_split_per_mille: 200,
        }
    }
}

impl SimulationConfig {
    pub fn from_toml(input: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(input)
    }

    pub fn to_toml(&self) -> Result<String, toml::ser::Error> {
        toml::to_string_pretty(self)
    }
}
