use rand_chacha::{ChaCha8Rng, rand_core::SeedableRng};
use serde::{Deserialize, Serialize};

use crate::{
    config::{LivingHistoryConfig, SimulationConfig},
    world::{Biome, RegionId, Resource, World},
};

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
    pub settlements: Vec<Settlement>,
    pub population_groups: Vec<PopulationGroup>,
    pub event_count: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settlement {
    pub id: SettlementId,
    pub name: String,
    pub region: RegionId,
    pub founded_month: u32,
    pub status: SettlementStatus,
    pub stability: i32,
}

pub type SettlementId = usize;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SettlementStatus {
    Active,
    Declining,
    Abandoned,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PopulationGroup {
    pub id: PopulationGroupId,
    pub name: String,
    pub region: RegionId,
    pub settlement: Option<SettlementId>,
    pub population: u64,
    pub subsistence: SubsistenceMode,
}

pub type PopulationGroupId = usize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SubsistenceMode {
    Foraging,
    Farming,
    Pastoral,
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
    pub event_type: EventType,
    pub severity: EventSeverity,
    pub tags: Vec<String>,
    pub subjects: Vec<EventSubject>,
    pub causes: Vec<String>,
    pub consequences: Vec<String>,
    pub summary: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EventType {
    SettlementFounded,
    SettlementGrowth,
    FoodPressure,
    Migration,
    SettlementDecline,
    EnvironmentalStress,
    SettlementAbandoned,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EventSubject {
    Region(RegionId),
    Settlement(SettlementId),
    PopulationGroup(PopulationGroupId),
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

impl SimulationState {
    pub fn total_population(&self) -> u64 {
        self.population_groups
            .iter()
            .map(|group| group.population)
            .sum()
    }

    pub fn settlement_population(&self, settlement_id: SettlementId) -> u64 {
        self.population_groups
            .iter()
            .filter(|group| group.settlement == Some(settlement_id))
            .map(|group| group.population)
            .sum()
    }

    pub fn effective_capacity(&self, region_id: RegionId, subsistence: SubsistenceMode) -> u64 {
        let region = &self.world.regions[region_id];
        let mut capacity = region.carrying_capacity as u64;

        capacity = match subsistence {
            SubsistenceMode::Farming => match region.biome {
                Biome::Grassland => capacity * 120 / 100,
                Biome::Forest | Biome::Rainforest => capacity * 90 / 100,
                Biome::Tundra | Biome::Desert => capacity * 55 / 100,
            },
            SubsistenceMode::Foraging => match region.biome {
                Biome::Forest | Biome::Rainforest => capacity * 105 / 100,
                Biome::Tundra | Biome::Desert => capacity * 75 / 100,
                Biome::Grassland => capacity * 85 / 100,
            },
            SubsistenceMode::Pastoral => match region.biome {
                Biome::Grassland => capacity * 115 / 100,
                Biome::Desert | Biome::Tundra => capacity * 80 / 100,
                Biome::Forest | Biome::Rainforest => capacity * 70 / 100,
            },
        };

        for resource in &region.resources {
            capacity = match (subsistence, resource) {
                (SubsistenceMode::Farming, Resource::Grain) => capacity * 125 / 100,
                (SubsistenceMode::Foraging, Resource::Fish) => capacity * 120 / 100,
                (SubsistenceMode::Pastoral, Resource::Horses) => capacity * 120 / 100,
                (_, Resource::Salt) => capacity * 105 / 100,
                (_, Resource::Timber) => capacity * 103 / 100,
                (_, Resource::Copper) => capacity,
                _ => capacity,
            };
        }

        capacity.max(1)
    }
}

impl Simulation {
    pub fn new(config: SimulationConfig, seed: SimulationSeed) -> Self {
        let world = World::generate(&config, seed);
        let (settlements, population_groups) = initialize_living_history(&world, &config);
        let population = population_groups.iter().map(|group| group.population).sum();

        Self {
            config,
            seed,
            state: SimulationState {
                month: 0,
                world,
                population,
                settlements,
                population_groups,
                event_count: 0,
            },
        }
    }

    pub fn tick_month(&mut self) -> Vec<SimulationEvent> {
        self.state.month += 1;
        let month = self.state.month;
        let mut events = Vec::new();

        let initial_settlement_count = self.state.settlements.len();
        for settlement_id in 0..initial_settlement_count {
            let Some(status) = self
                .state
                .settlements
                .get(settlement_id)
                .map(|settlement| settlement.status.clone())
            else {
                continue;
            };

            if status == SettlementStatus::Active {
                self.apply_growth(settlement_id, &mut events);
            }
            if status == SettlementStatus::Active || status == SettlementStatus::Declining {
                self.apply_environmental_stress(settlement_id, &mut events);
                self.apply_pressure(settlement_id, &mut events);
            }
        }

        if events.is_empty() {
            events.push(self.make_event(
                EventType::SettlementGrowth,
                EventSeverity::Note,
                vec!["settlement".to_string()],
                vec![],
                vec!["no settlement changed enough to report in this month".to_string()],
                vec!["history remained locally stable".to_string()],
                "Settlements remained stable as local pressures balanced.".to_string(),
            ));
        }

        debug_assert!(events.iter().all(|event| event.month == month));
        self.state.population = self.state.total_population();
        events
    }

    pub fn run_months(&mut self, months: u32) -> Vec<SimulationEvent> {
        (0..months).flat_map(|_| self.tick_month()).collect()
    }

    pub fn snapshot(&self) -> SimulationSnapshot {
        SimulationSnapshot {
            seed: self.seed,
            state: self.state.clone(),
        }
    }

    fn apply_growth(&mut self, settlement_id: SettlementId, events: &mut Vec<SimulationEvent>) {
        let growth_per_mille = self.config.living_history.monthly_growth_per_mille as u64;
        if growth_per_mille == 0 {
            return;
        }

        let group_ids: Vec<PopulationGroupId> = self
            .state
            .population_groups
            .iter()
            .filter(|group| group.settlement == Some(settlement_id))
            .map(|group| group.id)
            .collect();
        let mut total_growth = 0;
        for group_id in group_ids {
            let group = &mut self.state.population_groups[group_id];
            let growth = (group.population * growth_per_mille).div_ceil(1_000);
            group.population += growth;
            total_growth += growth;
        }

        if total_growth > 0 {
            let settlement = &self.state.settlements[settlement_id];
            events.push(self.make_event(
                EventType::SettlementGrowth,
                EventSeverity::Note,
                vec!["population".to_string(), "settlement".to_string()],
                vec![
                    EventSubject::Settlement(settlement.id),
                    EventSubject::Region(settlement.region),
                ],
                vec!["monthly subsistence surplus".to_string()],
                vec![format!("population increased by {total_growth}")],
                format!(
                    "{} grew by {total_growth} people as local subsistence held.",
                    settlement.name
                ),
            ));
        }
    }

    fn apply_pressure(&mut self, settlement_id: SettlementId, events: &mut Vec<SimulationEvent>) {
        let pressure = self.pressure_per_mille(settlement_id);
        let migration_threshold = self
            .config
            .living_history
            .migration_pressure_threshold_per_mille;
        let decline_threshold = self
            .config
            .living_history
            .decline_pressure_threshold_per_mille;

        if pressure < migration_threshold as u64 {
            return;
        }

        let settlement = self.state.settlements[settlement_id].clone();
        events.push(self.make_event(
            EventType::FoodPressure,
            EventSeverity::Important,
            vec!["food-pressure".to_string(), "settlement".to_string()],
            vec![
                EventSubject::Settlement(settlement.id),
                EventSubject::Region(settlement.region),
            ],
            vec![format!(
                "food pressure reached {pressure} per mille of carrying capacity"
            )],
            vec!["migration pressure rose".to_string()],
            format!(
                "Food pressure in {} rose above local carrying capacity.",
                settlement.name
            ),
        ));

        if let Some(target_region) = self.open_neighbor_region(settlement.region) {
            self.migrate_to_region(settlement_id, target_region, pressure, events);
        } else if pressure >= decline_threshold as u64 {
            self.decline_settlement(settlement_id, pressure, events);
        }
    }

    fn apply_environmental_stress(
        &mut self,
        settlement_id: SettlementId,
        events: &mut Vec<SimulationEvent>,
    ) {
        if self.state.month % 6 != 0 {
            return;
        }

        let settlement = self.state.settlements[settlement_id].clone();
        let region = &self.state.world.regions[settlement.region];
        let Some(group) = self
            .state
            .population_groups
            .iter()
            .find(|group| group.settlement == Some(settlement_id))
            .cloned()
        else {
            return;
        };
        let stress = stress_for(&region.biome, &group.subsistence);

        self.state.settlements[settlement_id].stability -= stress;
        events.push(self.make_event(
            EventType::EnvironmentalStress,
            EventSeverity::Important,
            vec![
                "environment".to_string(),
                "stress".to_string(),
                "settlement".to_string(),
            ],
            vec![
                EventSubject::Settlement(settlement.id),
                EventSubject::PopulationGroup(group.id),
                EventSubject::Region(settlement.region),
            ],
            vec![format!(
                "seasonal stress strained {:?} subsistence in {:?}",
                group.subsistence, region.biome
            )],
            vec![format!("{} lost {stress} stability", settlement.name)],
            format!(
                "Seasonal stress in {} strained {} and weakened local stability.",
                region.name, settlement.name
            ),
        ));
    }

    fn migrate_to_region(
        &mut self,
        origin_settlement_id: SettlementId,
        target_region: RegionId,
        pressure: u64,
        events: &mut Vec<SimulationEvent>,
    ) {
        let migrant_split = self.config.living_history.migrant_split_per_mille as u64;
        if migrant_split == 0 {
            return;
        }
        let origin_region = self.state.settlements[origin_settlement_id].region;
        let origin_group_id = self
            .state
            .population_groups
            .iter()
            .find(|group| group.settlement == Some(origin_settlement_id))
            .map(|group| group.id)
            .expect("origin settlement has a population group");
        let origin_population = self.state.population_groups[origin_group_id].population;
        let migrants = (origin_population * migrant_split)
            .div_ceil(1_000)
            .min(origin_population);
        if migrants == 0 {
            return;
        }

        self.state.population_groups[origin_group_id].population -= migrants;

        let settlement_id = self.state.settlements.len();
        let settlement_name = format!("{} Haven", self.state.world.regions[target_region].name);
        self.state.settlements.push(Settlement {
            id: settlement_id,
            name: settlement_name.clone(),
            region: target_region,
            founded_month: self.state.month,
            status: SettlementStatus::Active,
            stability: 100,
        });

        let group_id = self.state.population_groups.len();
        self.state.population_groups.push(PopulationGroup {
            id: group_id,
            name: format!("{} migrants", self.state.world.regions[target_region].name),
            region: target_region,
            settlement: Some(settlement_id),
            population: migrants,
            subsistence: SubsistenceMode::Farming,
        });

        events.push(self.make_event(
            EventType::Migration,
            EventSeverity::Important,
            vec!["migration".to_string(), "food-pressure".to_string()],
            vec![
                EventSubject::Settlement(origin_settlement_id),
                EventSubject::PopulationGroup(origin_group_id),
                EventSubject::Region(origin_region),
                EventSubject::Region(target_region),
            ],
            vec![format!("food pressure reached {pressure} per mille")],
            vec![format!("{migrants} people moved into a neighboring region")],
            format!(
                "Food pressure pushed {migrants} people from {} toward {}.",
                self.state.world.regions[origin_region].name,
                self.state.world.regions[target_region].name
            ),
        ));
        events.push(self.make_event(
            EventType::SettlementFounded,
            EventSeverity::Important,
            vec!["settlement".to_string(), "migration".to_string()],
            vec![
                EventSubject::Settlement(settlement_id),
                EventSubject::PopulationGroup(group_id),
                EventSubject::Region(target_region),
            ],
            vec!["migrants found open neighboring land".to_string()],
            vec![format!(
                "{settlement_name} was founded with {migrants} people"
            )],
            format!(
                "{settlement_name} was founded in {} by migrant households.",
                self.state.world.regions[target_region].name
            ),
        ));
    }

    fn decline_settlement(
        &mut self,
        settlement_id: SettlementId,
        pressure: u64,
        events: &mut Vec<SimulationEvent>,
    ) {
        let settlement = &mut self.state.settlements[settlement_id];
        settlement.status = SettlementStatus::Declining;
        settlement.stability -= 25;
        let settlement = settlement.clone();

        events.push(self.make_event(
            EventType::SettlementDecline,
            EventSeverity::Important,
            vec![
                "settlement".to_string(),
                "decline".to_string(),
                "food-pressure".to_string(),
            ],
            vec![
                EventSubject::Settlement(settlement.id),
                EventSubject::Region(settlement.region),
            ],
            vec![
                format!("food pressure reached {pressure} per mille"),
                "no open neighboring region could absorb migrants".to_string(),
            ],
            vec![format!("{} entered decline", settlement.name)],
            format!(
                "{} began to decline as pressure mounted with no open neighboring region.",
                settlement.name
            ),
        ));

        if settlement.stability <= 0 {
            self.abandon_settlement(settlement_id, events);
        }
    }

    fn abandon_settlement(
        &mut self,
        settlement_id: SettlementId,
        events: &mut Vec<SimulationEvent>,
    ) {
        let settlement = &mut self.state.settlements[settlement_id];
        settlement.status = SettlementStatus::Abandoned;
        settlement.stability = 0;
        let settlement = settlement.clone();

        let group_ids: Vec<PopulationGroupId> = self
            .state
            .population_groups
            .iter()
            .filter(|group| group.settlement == Some(settlement_id))
            .map(|group| group.id)
            .collect();
        let mut population_loss = 0;
        for group_id in group_ids {
            let group = &mut self.state.population_groups[group_id];
            let loss = group.population / 2;
            population_loss += loss;
            group.population -= loss;
            group.settlement = None;
        }

        events.push(self.make_event(
            EventType::SettlementAbandoned,
            EventSeverity::Important,
            vec![
                "settlement".to_string(),
                "abandonment".to_string(),
                "population-loss".to_string(),
            ],
            vec![
                EventSubject::Settlement(settlement.id),
                EventSubject::Region(settlement.region),
            ],
            vec![
                "stability reached zero".to_string(),
                "no open neighboring region could absorb migrants".to_string(),
            ],
            vec![
                format!("{} was abandoned", settlement.name),
                format!("population loss of {population_loss} people"),
            ],
            format!(
                "{} was abandoned after sustained pressure caused a population loss of {population_loss} people.",
                settlement.name
            ),
        ));
    }

    fn pressure_per_mille(&self, settlement_id: SettlementId) -> u64 {
        let settlement = &self.state.settlements[settlement_id];
        let capacity = self
            .state
            .population_groups
            .iter()
            .filter(|group| group.settlement == Some(settlement_id))
            .map(|group| {
                self.state
                    .effective_capacity(settlement.region, group.subsistence)
            })
            .max()
            .unwrap_or_else(|| {
                self.state.world.regions[settlement.region].carrying_capacity as u64
            });
        if capacity == 0 {
            return u64::MAX;
        }
        self.state.settlement_population(settlement_id) * 1_000 / capacity
    }

    fn open_neighbor_region(&self, region: RegionId) -> Option<RegionId> {
        self.state.world.regions[region]
            .neighbors
            .iter()
            .copied()
            .find(|neighbor| {
                self.state
                    .settlements
                    .iter()
                    .all(|settlement| settlement.region != *neighbor)
            })
    }

    fn make_event(
        &mut self,
        event_type: EventType,
        severity: EventSeverity,
        tags: Vec<String>,
        subjects: Vec<EventSubject>,
        causes: Vec<String>,
        consequences: Vec<String>,
        summary: String,
    ) -> SimulationEvent {
        self.state.event_count += 1;
        SimulationEvent {
            id: self.state.event_count,
            month: self.state.month,
            event_type,
            severity,
            tags,
            subjects,
            causes,
            consequences,
            summary,
        }
    }
}

fn stress_for(biome: &Biome, subsistence: &SubsistenceMode) -> i32 {
    match (biome, subsistence) {
        (Biome::Desert | Biome::Tundra, SubsistenceMode::Farming) => 30,
        (Biome::Rainforest, SubsistenceMode::Pastoral) => 20,
        (Biome::Desert, SubsistenceMode::Foraging) => 15,
        _ => 5,
    }
}

fn initialize_living_history(
    world: &World,
    config: &SimulationConfig,
) -> (Vec<Settlement>, Vec<PopulationGroup>) {
    let settings = &config.living_history;
    let settlement_count = settings.initial_settlements.max(1).min(world.regions.len());
    let mut settlements = Vec::with_capacity(settlement_count);
    let mut population_groups = Vec::with_capacity(settlement_count);

    for region in world.regions.iter().take(settlement_count) {
        let settlement_id = settlements.len();
        let population = initial_population(region.carrying_capacity, settings);
        settlements.push(Settlement {
            id: settlement_id,
            name: format!("{} Hearth", region.name),
            region: region.id,
            founded_month: 0,
            status: SettlementStatus::Active,
            stability: 100,
        });
        population_groups.push(PopulationGroup {
            id: settlement_id,
            name: format!("{} households", region.name),
            region: region.id,
            settlement: Some(settlement_id),
            population,
            subsistence: SubsistenceMode::Farming,
        });
    }

    (settlements, population_groups)
}

fn initial_population(carrying_capacity: u32, settings: &LivingHistoryConfig) -> u64 {
    ((carrying_capacity as u64 * settings.initial_population_per_mille as u64) / 1_000).max(1)
}

pub(crate) fn seeded_rng(seed: SimulationSeed, stream: u64) -> ChaCha8Rng {
    ChaCha8Rng::seed_from_u64(seed.value() ^ stream.rotate_left(17))
}
