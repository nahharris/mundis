use rand::{Rng, seq::IndexedRandom};
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};

use crate::{
    config::SimulationConfig,
    simulation::{SimulationSeed, seeded_rng},
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct World {
    pub regions: Vec<Region>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Region {
    pub id: RegionId,
    pub name: String,
    pub climate: Climate,
    pub biome: Biome,
    pub resources: Vec<Resource>,
    pub carrying_capacity: u32,
    pub neighbors: Vec<RegionId>,
}

pub type RegionId = usize;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Climate {
    Arctic,
    Temperate,
    Arid,
    Tropical,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Biome {
    Tundra,
    Forest,
    Grassland,
    Desert,
    Rainforest,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Resource {
    Grain,
    Fish,
    Timber,
    Copper,
    Horses,
    Salt,
}

impl World {
    pub fn generate(config: &SimulationConfig, seed: SimulationSeed) -> Self {
        let count = config.world.regions.max(2);
        let mut rng = seeded_rng(seed, 0xA11C_E000);
        let mut regions = Vec::with_capacity(count);

        for index in 0..count {
            let climate = random_climate(&mut rng);
            let biome = biome_for(&climate, &mut rng);
            let mut neighbors = Vec::new();

            if index > 0 {
                neighbors.push(index - 1);
            }
            if index + 1 < count {
                neighbors.push(index + 1);
            }
            if count > 3 && index % 3 == 0 {
                neighbors.push((index + 2) % count);
            }
            neighbors.sort_unstable();
            neighbors.dedup();

            regions.push(Region {
                id: index,
                name: generate_name(&mut rng),
                carrying_capacity: carrying_capacity(&biome, &mut rng),
                resources: resources_for(&biome, &mut rng),
                climate,
                biome,
                neighbors,
            });
        }

        Self { regions }
    }

    pub fn is_connected(&self) -> bool {
        if self.regions.is_empty() {
            return true;
        }

        let mut seen = vec![false; self.regions.len()];
        let mut stack = vec![0usize];

        while let Some(index) = stack.pop() {
            if seen[index] {
                continue;
            }
            seen[index] = true;

            for neighbor in &self.regions[index].neighbors {
                let neighbor = *neighbor;
                if neighbor < self.regions.len() && !seen[neighbor] {
                    stack.push(neighbor);
                }
            }
        }

        seen.into_iter().all(|value| value)
    }
}

fn random_climate(rng: &mut ChaCha8Rng) -> Climate {
    match rng.random_range(0..4) {
        0 => Climate::Arctic,
        1 => Climate::Temperate,
        2 => Climate::Arid,
        _ => Climate::Tropical,
    }
}

fn biome_for(climate: &Climate, rng: &mut ChaCha8Rng) -> Biome {
    match climate {
        Climate::Arctic => Biome::Tundra,
        Climate::Temperate => [Biome::Forest, Biome::Grassland]
            .choose(rng)
            .expect("biomes")
            .clone(),
        Climate::Arid => Biome::Desert,
        Climate::Tropical => [Biome::Rainforest, Biome::Grassland]
            .choose(rng)
            .expect("biomes")
            .clone(),
    }
}

fn carrying_capacity(biome: &Biome, rng: &mut ChaCha8Rng) -> u32 {
    let base = match biome {
        Biome::Tundra => 600,
        Biome::Forest => 2_400,
        Biome::Grassland => 3_200,
        Biome::Desert => 500,
        Biome::Rainforest => 2_000,
    };
    base + rng.random_range(0..500)
}

fn resources_for(biome: &Biome, rng: &mut ChaCha8Rng) -> Vec<Resource> {
    let primary = match biome {
        Biome::Tundra => Resource::Fish,
        Biome::Forest => Resource::Timber,
        Biome::Grassland => Resource::Grain,
        Biome::Desert => Resource::Salt,
        Biome::Rainforest => Resource::Timber,
    };
    let secondary = [
        Resource::Copper,
        Resource::Horses,
        Resource::Fish,
        Resource::Salt,
    ]
    .choose(rng)
    .expect("resources")
    .clone();

    if primary == secondary {
        vec![primary]
    } else {
        vec![primary, secondary]
    }
}

fn generate_name(rng: &mut ChaCha8Rng) -> String {
    const STARTS: &[&str] = &["Aru", "Bel", "Cair", "Doma", "Esh", "Fara", "Galen", "Haru"];
    const ENDS: &[&str] = &["mar", "neth", "sai", "tor", "ven", "ka", "rul", "dun"];

    format!(
        "{}{}",
        STARTS.choose(rng).expect("starts"),
        ENDS.choose(rng).expect("ends")
    )
}
