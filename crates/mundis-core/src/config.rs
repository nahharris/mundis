use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SimulationConfig {
    pub months: u32,
    pub world: WorldSize,
    pub living_history: LivingHistoryConfig,
    pub bias: SimulationBias,
    pub output: OutputConfig,
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

impl Default for SimulationConfig {
    fn default() -> Self {
        Self {
            months: 120,
            world: WorldSize { regions: 6 },
            living_history: LivingHistoryConfig::default(),
            bias: SimulationBias::Plausible,
            output: OutputConfig {
                verbosity: OutputVerbosity::Chronicle,
            },
        }
    }
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
