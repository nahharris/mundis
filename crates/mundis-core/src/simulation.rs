use rand::Rng;
use rand_chacha::{ChaCha8Rng, rand_core::SeedableRng};
use serde::{Deserialize, Serialize};

use crate::{config::SimulationConfig, world::World};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SimulationSeed(u64);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Simulation {
    config: SimulationConfig,
    seed: SimulationSeed,
    state: SimulationState,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SimulationState {
    pub month: u32,
    pub world: World,
    pub population: u64,
    pub event_count: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SimulationSnapshot {
    pub seed: SimulationSeed,
    pub state: SimulationState,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SimulationEvent {
    pub id: u64,
    pub month: u32,
    pub severity: EventSeverity,
    pub tags: Vec<String>,
    pub summary: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EventSeverity {
    Note,
    Important,
}

impl SimulationSeed {
    pub fn from_u64(value: u64) -> Self {
        Self(value)
    }

    pub fn value(self) -> u64 {
        self.0
    }
}

impl Simulation {
    pub fn new(config: SimulationConfig, seed: SimulationSeed) -> Self {
        let world = World::generate(&config, seed);
        let population = world
            .regions
            .iter()
            .map(|region| region.carrying_capacity as u64 / 4)
            .sum();

        Self {
            config,
            seed,
            state: SimulationState {
                month: 0,
                world,
                population,
                event_count: 0,
            },
        }
    }

    pub fn tick_month(&mut self) -> SimulationEvent {
        self.state.month += 1;
        self.state.event_count += 1;

        let mut rng = seeded_rng(self.seed, self.state.month as u64);
        let region = &self.state.world.regions[rng.random_range(0..self.state.world.regions.len())];
        let growth = rng.random_range(0..=region.carrying_capacity as u64 / 120);
        self.state.population += growth;

        let (severity, verb) = if self.state.month % 12 == 0 {
            (EventSeverity::Important, "reshaped")
        } else {
            (EventSeverity::Note, "stirred")
        };

        SimulationEvent {
            id: self.state.event_count,
            month: self.state.month,
            severity,
            tags: vec!["population".to_string(), "region".to_string()],
            summary: format!(
                "{} {} as population pressure rose by {} people.",
                region.name, verb, growth
            ),
        }
    }

    pub fn run_months(&mut self, months: u32) -> Vec<SimulationEvent> {
        (0..months).map(|_| self.tick_month()).collect()
    }

    pub fn snapshot(&self) -> SimulationSnapshot {
        SimulationSnapshot {
            seed: self.seed,
            state: self.state.clone(),
        }
    }
}

pub(crate) fn seeded_rng(seed: SimulationSeed, stream: u64) -> ChaCha8Rng {
    ChaCha8Rng::seed_from_u64(seed.value() ^ stream.rotate_left(17))
}
