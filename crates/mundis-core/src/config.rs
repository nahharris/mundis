use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SimulationConfig {
    pub months: u32,
    pub world: WorldSize,
    pub bias: SimulationBias,
    pub output: OutputConfig,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldSize {
    pub regions: usize,
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
            bias: SimulationBias::Plausible,
            output: OutputConfig {
                verbosity: OutputVerbosity::Chronicle,
            },
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
