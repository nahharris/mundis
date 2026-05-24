use std::{collections::HashMap, error::Error, fmt};

use serde::Deserialize;

use crate::{
    civilization::{Culture, CultureTrait, Institution, NamingTradition, Polity, PolityStatus},
    config::{
        CivilizationConfig, LivingHistoryConfig, OutputConfig, SimulationBias, SimulationConfig,
        WorldSize,
    },
    simulation::{
        EventSeverity, EventSubject, EventType, PopulationGroup, Settlement, SettlementStatus,
        SimulationEvent, SimulationSeed, SubsistenceMode, initialize_living_history,
    },
    world::{Biome, Climate, Region, Resource, World},
};

#[derive(Clone, Debug)]
pub struct CompiledScenario {
    pub config: SimulationConfig,
    pub seed: SimulationSeed,
    pub world: World,
    pub settlements: Vec<Settlement>,
    pub population_groups: Vec<PopulationGroup>,
    pub cultures: Vec<Culture>,
    pub polities: Vec<Polity>,
    pub background_events: Vec<SimulationEvent>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ScenarioConfig {
    #[serde(default)]
    simulation: ScenarioSimulationConfig,
    #[serde(default)]
    regions: Vec<AuthoredRegion>,
    #[serde(default)]
    cultures: Vec<AuthoredCulture>,
    #[serde(default)]
    settlements: Vec<AuthoredSettlement>,
    #[serde(default)]
    population_groups: Vec<AuthoredPopulationGroup>,
    #[serde(default)]
    polities: Vec<AuthoredPolity>,
    #[serde(default)]
    background_events: Vec<AuthoredBackgroundEvent>,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct ScenarioSimulationConfig {
    months: Option<u32>,
    world: Option<PartialWorldSize>,
    civilization: Option<PartialCivilizationConfig>,
    living_history: Option<PartialLivingHistoryConfig>,
    bias: Option<SimulationBias>,
    output: Option<PartialOutputConfig>,
}

#[derive(Clone, Debug, Deserialize)]
struct PartialWorldSize {
    regions: Option<usize>,
}

#[derive(Clone, Debug, Deserialize)]
struct PartialLivingHistoryConfig {
    initial_settlements: Option<usize>,
    initial_population_per_mille: Option<u32>,
    monthly_growth_per_mille: Option<u32>,
    migration_pressure_threshold_per_mille: Option<u32>,
    decline_pressure_threshold_per_mille: Option<u32>,
    migrant_split_per_mille: Option<u32>,
}

#[derive(Clone, Debug, Deserialize)]
struct PartialCivilizationConfig {
    enabled: Option<bool>,
    polity_foundation_population: Option<u64>,
    expansion_pressure_threshold_per_mille: Option<u32>,
    trade_interval_months: Option<u32>,
    tension_interval_months: Option<u32>,
    cultural_drift_interval_months: Option<u32>,
    collapse_cohesion_threshold: Option<i32>,
    alliance_interval_months: Option<u32>,
    war_interval_months: Option<u32>,
    assimilation_interval_months: Option<u32>,
    fragmentation_interval_months: Option<u32>,
    succession_interval_months: Option<u32>,
    war_tension_threshold: Option<i32>,
    fragmentation_cohesion_threshold: Option<i32>,
    war_end_score_threshold: Option<i32>,
}

#[derive(Clone, Debug, Deserialize)]
struct PartialOutputConfig {
    verbosity: Option<crate::config::OutputVerbosity>,
}

#[derive(Clone, Debug, Deserialize)]
struct AuthoredRegion {
    id: String,
    name: String,
    climate: Climate,
    biome: Biome,
    resources: Vec<Resource>,
    carrying_capacity: u32,
    #[serde(default)]
    neighbors: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
struct AuthoredCulture {
    id: String,
    name: String,
    origin_region: String,
    #[serde(default)]
    traits: Vec<CultureTrait>,
}

#[derive(Clone, Debug, Deserialize)]
struct AuthoredSettlement {
    id: String,
    name: String,
    region: String,
    population: u64,
    culture: Option<String>,
    #[serde(default = "default_stability")]
    stability: i32,
}

#[derive(Clone, Debug, Deserialize)]
struct AuthoredPopulationGroup {
    id: String,
    name: String,
    region: String,
    settlement: Option<String>,
    population: u64,
    #[serde(default = "default_subsistence")]
    subsistence: SubsistenceMode,
    culture: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
struct AuthoredPolity {
    id: String,
    name: String,
    primary_culture: String,
    capital: String,
    #[serde(default)]
    controlled_settlements: Vec<String>,
    #[serde(default)]
    controlled_regions: Vec<String>,
    #[serde(default = "default_cohesion")]
    cohesion: i32,
}

#[derive(Clone, Debug, Deserialize)]
struct AuthoredBackgroundEvent {
    id: String,
    summary: String,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    regions: Vec<String>,
    #[serde(default)]
    settlements: Vec<String>,
    #[serde(default)]
    population_groups: Vec<String>,
    #[serde(default)]
    cultures: Vec<String>,
    #[serde(default)]
    polities: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScenarioError {
    message: String,
}

impl ScenarioConfig {
    pub fn from_toml(input: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(input)
    }

    pub fn compile(
        &self,
        mut base: SimulationConfig,
        seed: SimulationSeed,
    ) -> Result<CompiledScenario, ScenarioError> {
        self.simulation.apply_to(&mut base);

        let generated_world = World::generate(&base, seed);
        let world = self.compile_world(&base, generated_world)?;
        let (mut settlements, mut population_groups, mut cultures) =
            initialize_living_history(&world, &base);
        let mut polities = Vec::new();

        let region_ids = self.region_ids(&world)?;
        if !self.cultures.is_empty() || !self.settlements.is_empty() {
            cultures = self.compile_cultures(&region_ids)?;
        }
        let culture_ids = ids_for(&cultures, self.cultures.iter().map(|culture| &culture.id));

        if !self.settlements.is_empty() {
            settlements = self.compile_settlements(&region_ids)?;
            population_groups =
                self.compile_groups(&region_ids, &culture_ids, &settlements, &self.settlements)?;
        }
        let settlement_ids = ids_for(
            &settlements,
            self.settlements.iter().map(|settlement| &settlement.id),
        );
        if !self.population_groups.is_empty() {
            population_groups =
                self.compile_authored_groups(&region_ids, &settlement_ids, &culture_ids)?;
        }
        let population_group_ids = ids_for(
            &population_groups,
            self.population_groups.iter().map(|group| &group.id),
        );

        if !self.polities.is_empty() {
            polities = self.compile_polities(&region_ids, &settlement_ids, &culture_ids)?;
            for polity in &polities {
                for settlement_id in &polity.controlled_settlements {
                    settlements[*settlement_id].polity = Some(polity.id);
                }
            }
        }
        let polity_ids = ids_for(&polities, self.polities.iter().map(|polity| &polity.id));
        let background_events = self.compile_background_events(
            &region_ids,
            &settlement_ids,
            &population_group_ids,
            &culture_ids,
            &polity_ids,
        )?;

        validate_population(&population_groups)?;

        Ok(CompiledScenario {
            config: base,
            seed,
            world,
            settlements,
            population_groups,
            cultures,
            polities,
            background_events,
        })
    }

    fn compile_world(
        &self,
        config: &SimulationConfig,
        mut generated: World,
    ) -> Result<World, ScenarioError> {
        reject_duplicates(
            "region",
            self.regions.iter().map(|region| region.id.as_str()),
        )?;
        let authored_count = self.regions.len();
        let target_count = config.world.regions.max(authored_count).max(2);
        generated.regions.truncate(target_count);

        let mut authored_index = HashMap::new();
        for (index, region) in self.regions.iter().enumerate() {
            authored_index.insert(region.id.clone(), index);
        }

        for (index, authored) in self.regions.iter().enumerate() {
            generated.regions[index] = Region {
                id: index,
                name: authored.name.clone(),
                climate: authored.climate.clone(),
                biome: authored.biome.clone(),
                resources: authored.resources.clone(),
                carrying_capacity: authored.carrying_capacity,
                neighbors: Vec::new(),
            };
        }

        for (index, authored) in self.regions.iter().enumerate() {
            if authored.neighbors.is_empty() {
                continue;
            }
            let mut neighbors = Vec::new();
            for neighbor in &authored.neighbors {
                let Some(neighbor_id) = authored_index.get(neighbor).copied() else {
                    return Err(ScenarioError::new(format!(
                        "region '{}': unknown neighbor '{}'",
                        authored.id, neighbor
                    )));
                };
                neighbors.push(neighbor_id);
            }
            neighbors.sort_unstable();
            neighbors.dedup();
            generated.regions[index].neighbors = neighbors;
        }

        for index in 0..generated.regions.len() {
            let neighbors = generated.regions[index].neighbors.clone();
            for neighbor in neighbors {
                if neighbor < generated.regions.len()
                    && !generated.regions[neighbor].neighbors.contains(&index)
                {
                    generated.regions[neighbor].neighbors.push(index);
                    generated.regions[neighbor].neighbors.sort_unstable();
                }
            }
        }

        Ok(generated)
    }

    fn compile_cultures(
        &self,
        region_ids: &HashMap<String, usize>,
    ) -> Result<Vec<Culture>, ScenarioError> {
        reject_duplicates(
            "culture",
            self.cultures.iter().map(|culture| culture.id.as_str()),
        )?;
        self.cultures
            .iter()
            .enumerate()
            .map(|(id, culture)| {
                let origin_region = lookup(
                    region_ids,
                    "culture",
                    &culture.id,
                    "origin_region",
                    &culture.origin_region,
                )?;
                Ok(Culture {
                    id,
                    name: culture.name.clone(),
                    origin_region,
                    traits: culture.traits.clone(),
                    naming: NamingTradition::PrefixSuffix {
                        starts: vec![culture.name.chars().take(3).collect()],
                        ends: vec!["mar".to_string(), "ven".to_string()],
                    },
                    drift: 0,
                })
            })
            .collect()
    }

    fn compile_settlements(
        &self,
        region_ids: &HashMap<String, usize>,
    ) -> Result<Vec<Settlement>, ScenarioError> {
        reject_duplicates(
            "settlement",
            self.settlements
                .iter()
                .map(|settlement| settlement.id.as_str()),
        )?;
        self.settlements
            .iter()
            .enumerate()
            .map(|(id, settlement)| {
                let region = lookup(
                    region_ids,
                    "settlement",
                    &settlement.id,
                    "region",
                    &settlement.region,
                )?;
                Ok(Settlement {
                    id,
                    name: settlement.name.clone(),
                    region,
                    founded_month: 0,
                    status: SettlementStatus::Active,
                    stability: settlement.stability,
                    polity: None,
                })
            })
            .collect()
    }

    fn compile_groups(
        &self,
        region_ids: &HashMap<String, usize>,
        culture_ids: &HashMap<String, usize>,
        settlements: &[Settlement],
        authored_settlements: &[AuthoredSettlement],
    ) -> Result<Vec<PopulationGroup>, ScenarioError> {
        authored_settlements
            .iter()
            .enumerate()
            .map(|(id, settlement)| {
                let culture = optional_lookup(
                    culture_ids,
                    "settlement",
                    &settlement.id,
                    "culture",
                    settlement.culture.as_deref(),
                )?;
                let region = lookup(
                    region_ids,
                    "settlement",
                    &settlement.id,
                    "region",
                    &settlement.region,
                )?;
                Ok(PopulationGroup {
                    id,
                    name: format!("{} households", settlement.name),
                    region,
                    settlement: Some(settlements[id].id),
                    population: settlement.population,
                    subsistence: SubsistenceMode::Farming,
                    culture,
                })
            })
            .collect()
    }

    fn compile_authored_groups(
        &self,
        region_ids: &HashMap<String, usize>,
        settlement_ids: &HashMap<String, usize>,
        culture_ids: &HashMap<String, usize>,
    ) -> Result<Vec<PopulationGroup>, ScenarioError> {
        reject_duplicates(
            "population_group",
            self.population_groups.iter().map(|group| group.id.as_str()),
        )?;
        self.population_groups
            .iter()
            .enumerate()
            .map(|(id, group)| {
                let region = lookup(
                    region_ids,
                    "population_group",
                    &group.id,
                    "region",
                    &group.region,
                )?;
                let settlement = optional_lookup(
                    settlement_ids,
                    "population_group",
                    &group.id,
                    "settlement",
                    group.settlement.as_deref(),
                )?;
                let culture = optional_lookup(
                    culture_ids,
                    "population_group",
                    &group.id,
                    "culture",
                    group.culture.as_deref(),
                )?;
                Ok(PopulationGroup {
                    id,
                    name: group.name.clone(),
                    region,
                    settlement,
                    population: group.population,
                    subsistence: group.subsistence,
                    culture,
                })
            })
            .collect()
    }

    fn compile_polities(
        &self,
        region_ids: &HashMap<String, usize>,
        settlement_ids: &HashMap<String, usize>,
        culture_ids: &HashMap<String, usize>,
    ) -> Result<Vec<Polity>, ScenarioError> {
        reject_duplicates(
            "polity",
            self.polities.iter().map(|polity| polity.id.as_str()),
        )?;
        self.polities
            .iter()
            .enumerate()
            .map(|(id, polity)| {
                let primary_culture = lookup(
                    culture_ids,
                    "polity",
                    &polity.id,
                    "primary_culture",
                    &polity.primary_culture,
                )?;
                let capital = lookup(
                    settlement_ids,
                    "polity",
                    &polity.id,
                    "capital",
                    &polity.capital,
                )?;
                let mut controlled_settlements = Vec::new();
                if polity.controlled_settlements.is_empty() {
                    controlled_settlements.push(capital);
                } else {
                    for settlement in &polity.controlled_settlements {
                        controlled_settlements.push(lookup(
                            settlement_ids,
                            "polity",
                            &polity.id,
                            "controlled_settlements",
                            settlement,
                        )?);
                    }
                }
                controlled_settlements.sort_unstable();
                controlled_settlements.dedup();

                let mut controlled_regions = Vec::new();
                for region in &polity.controlled_regions {
                    controlled_regions.push(lookup(
                        region_ids,
                        "polity",
                        &polity.id,
                        "controlled_regions",
                        region,
                    )?);
                }
                controlled_regions.sort_unstable();
                controlled_regions.dedup();

                Ok(Polity {
                    id,
                    name: polity.name.clone(),
                    status: PolityStatus::Active,
                    primary_culture,
                    capital,
                    controlled_settlements,
                    controlled_regions,
                    institutions: vec![Institution::Council],
                    succession_count: 0,
                    parent: None,
                    cohesion: polity.cohesion,
                })
            })
            .collect()
    }

    fn compile_background_events(
        &self,
        region_ids: &HashMap<String, usize>,
        settlement_ids: &HashMap<String, usize>,
        population_group_ids: &HashMap<String, usize>,
        culture_ids: &HashMap<String, usize>,
        polity_ids: &HashMap<String, usize>,
    ) -> Result<Vec<SimulationEvent>, ScenarioError> {
        reject_duplicates(
            "background_event",
            self.background_events.iter().map(|event| event.id.as_str()),
        )?;
        self.background_events
            .iter()
            .enumerate()
            .map(|(index, event)| {
                let mut subjects = Vec::new();
                for region in &event.regions {
                    subjects.push(EventSubject::Region(lookup(
                        region_ids,
                        "background_event",
                        &event.id,
                        "regions",
                        region,
                    )?));
                }
                for settlement in &event.settlements {
                    subjects.push(EventSubject::Settlement(lookup(
                        settlement_ids,
                        "background_event",
                        &event.id,
                        "settlements",
                        settlement,
                    )?));
                }
                for group in &event.population_groups {
                    subjects.push(EventSubject::PopulationGroup(lookup(
                        population_group_ids,
                        "background_event",
                        &event.id,
                        "population_groups",
                        group,
                    )?));
                }
                for culture in &event.cultures {
                    subjects.push(EventSubject::Culture(lookup(
                        culture_ids,
                        "background_event",
                        &event.id,
                        "cultures",
                        culture,
                    )?));
                }
                for polity in &event.polities {
                    subjects.push(EventSubject::Polity(lookup(
                        polity_ids,
                        "background_event",
                        &event.id,
                        "polities",
                        polity,
                    )?));
                }
                Ok(SimulationEvent {
                    id: index as u64 + 1,
                    month: 0,
                    event_type: EventType::BackgroundEvent,
                    severity: EventSeverity::Important,
                    tags: event.tags.clone(),
                    subjects,
                    causes: vec!["authored scenario background".to_string()],
                    consequences: vec!["initial conditions were established".to_string()],
                    summary: event.summary.clone(),
                })
            })
            .collect()
    }

    fn region_ids(&self, world: &World) -> Result<HashMap<String, usize>, ScenarioError> {
        let mut ids = HashMap::new();
        for (index, region) in self.regions.iter().enumerate() {
            ids.insert(region.id.clone(), index);
        }
        for region in &world.regions {
            ids.entry(region.name.clone()).or_insert(region.id);
        }
        Ok(ids)
    }
}

impl ScenarioSimulationConfig {
    fn apply_to(&self, config: &mut SimulationConfig) {
        if let Some(months) = self.months {
            config.months = months;
        }
        if let Some(world) = &self.world {
            world.apply_to(&mut config.world);
        }
        if let Some(civilization) = &self.civilization {
            civilization.apply_to(&mut config.civilization);
        }
        if let Some(living_history) = &self.living_history {
            living_history.apply_to(&mut config.living_history);
        }
        if let Some(bias) = &self.bias {
            config.bias = bias.clone();
        }
        if let Some(output) = &self.output {
            output.apply_to(&mut config.output);
        }
    }
}

impl PartialWorldSize {
    fn apply_to(&self, config: &mut WorldSize) {
        if let Some(regions) = self.regions {
            config.regions = regions;
        }
    }
}

impl PartialLivingHistoryConfig {
    fn apply_to(&self, config: &mut LivingHistoryConfig) {
        if let Some(value) = self.initial_settlements {
            config.initial_settlements = value;
        }
        if let Some(value) = self.initial_population_per_mille {
            config.initial_population_per_mille = value;
        }
        if let Some(value) = self.monthly_growth_per_mille {
            config.monthly_growth_per_mille = value;
        }
        if let Some(value) = self.migration_pressure_threshold_per_mille {
            config.migration_pressure_threshold_per_mille = value;
        }
        if let Some(value) = self.decline_pressure_threshold_per_mille {
            config.decline_pressure_threshold_per_mille = value;
        }
        if let Some(value) = self.migrant_split_per_mille {
            config.migrant_split_per_mille = value;
        }
    }
}

impl PartialCivilizationConfig {
    fn apply_to(&self, config: &mut CivilizationConfig) {
        if let Some(value) = self.enabled {
            config.enabled = value;
        }
        if let Some(value) = self.polity_foundation_population {
            config.polity_foundation_population = value;
        }
        if let Some(value) = self.expansion_pressure_threshold_per_mille {
            config.expansion_pressure_threshold_per_mille = value;
        }
        if let Some(value) = self.trade_interval_months {
            config.trade_interval_months = value;
        }
        if let Some(value) = self.tension_interval_months {
            config.tension_interval_months = value;
        }
        if let Some(value) = self.cultural_drift_interval_months {
            config.cultural_drift_interval_months = value;
        }
        if let Some(value) = self.collapse_cohesion_threshold {
            config.collapse_cohesion_threshold = value;
        }
        if let Some(value) = self.alliance_interval_months {
            config.alliance_interval_months = value;
        }
        if let Some(value) = self.war_interval_months {
            config.war_interval_months = value;
        }
        if let Some(value) = self.assimilation_interval_months {
            config.assimilation_interval_months = value;
        }
        if let Some(value) = self.fragmentation_interval_months {
            config.fragmentation_interval_months = value;
        }
        if let Some(value) = self.succession_interval_months {
            config.succession_interval_months = value;
        }
        if let Some(value) = self.war_tension_threshold {
            config.war_tension_threshold = value;
        }
        if let Some(value) = self.fragmentation_cohesion_threshold {
            config.fragmentation_cohesion_threshold = value;
        }
        if let Some(value) = self.war_end_score_threshold {
            config.war_end_score_threshold = value;
        }
    }
}

impl PartialOutputConfig {
    fn apply_to(&self, config: &mut OutputConfig) {
        if let Some(value) = &self.verbosity {
            config.verbosity = value.clone();
        }
    }
}

impl ScenarioError {
    fn new(message: String) -> Self {
        Self { message }
    }
}

impl fmt::Display for ScenarioError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for ScenarioError {}

fn reject_duplicates<'a>(
    kind: &str,
    ids: impl Iterator<Item = &'a str>,
) -> Result<(), ScenarioError> {
    let mut seen = HashMap::new();
    for id in ids {
        if seen.insert(id.to_string(), ()).is_some() {
            return Err(ScenarioError::new(format!("duplicate {kind} id '{id}'")));
        }
    }
    Ok(())
}

fn lookup(
    ids: &HashMap<String, usize>,
    kind: &str,
    object: &str,
    field: &str,
    reference: &str,
) -> Result<usize, ScenarioError> {
    ids.get(reference).copied().ok_or_else(|| {
        ScenarioError::new(format!("{kind} '{object}': unknown {field} '{reference}'"))
    })
}

fn optional_lookup(
    ids: &HashMap<String, usize>,
    kind: &str,
    object: &str,
    field: &str,
    reference: Option<&str>,
) -> Result<Option<usize>, ScenarioError> {
    reference
        .map(|reference| lookup(ids, kind, object, field, reference))
        .transpose()
}

fn ids_for<T>(
    items: &[T],
    authored_ids: impl Iterator<Item = impl AsRef<str>>,
) -> HashMap<String, usize> {
    authored_ids
        .enumerate()
        .filter(|(index, _)| *index < items.len())
        .map(|(index, id)| (id.as_ref().to_string(), index))
        .collect()
}

fn validate_population(groups: &[PopulationGroup]) -> Result<(), ScenarioError> {
    if groups.is_empty() {
        return Err(ScenarioError::new(
            "impossible population state: at least one population group is required".to_string(),
        ));
    }
    if groups.iter().any(|group| group.population == 0) {
        return Err(ScenarioError::new(
            "impossible population state: population groups must be greater than zero".to_string(),
        ));
    }
    Ok(())
}

fn default_stability() -> i32 {
    100
}

fn default_subsistence() -> SubsistenceMode {
    SubsistenceMode::Farming
}

fn default_cohesion() -> i32 {
    100
}
