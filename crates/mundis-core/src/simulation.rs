use rand_chacha::{ChaCha8Rng, rand_core::SeedableRng};
use serde::{Deserialize, Serialize};

use crate::{
    civilization::{
        Alliance, AllianceStatus, Culture, CultureId, CultureTrait, Institution, NamingTradition,
        Polity, PolityId, PolityStatus, Rivalry, RivalryId, TradeLink, Treaty, TreatyTerm, War,
        WarId, WarStatus,
    },
    config::{LivingHistoryConfig, SimulationConfig},
    scenario::CompiledScenario,
    world::{Biome, RegionId, Resource, World},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SimulationSeed(u64);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Simulation {
    config: SimulationConfig,
    seed: SimulationSeed,
    state: SimulationState,
    pending_events: Vec<SimulationEvent>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SimulationState {
    pub month: u32,
    pub world: World,
    pub population: u64,
    pub settlements: Vec<Settlement>,
    pub population_groups: Vec<PopulationGroup>,
    pub cultures: Vec<Culture>,
    pub polities: Vec<Polity>,
    pub trade_links: Vec<TradeLink>,
    pub rivalries: Vec<Rivalry>,
    pub alliances: Vec<Alliance>,
    pub wars: Vec<War>,
    pub treaties: Vec<Treaty>,
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
    pub polity: Option<PolityId>,
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
    pub culture: Option<CultureId>,
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
    PolityFounded,
    PolityExpanded,
    TradeLinkFormed,
    BorderTension,
    CultureDrift,
    PolityCollapse,
    AllianceFormed,
    WarDeclared,
    WarEnded,
    TreatySigned,
    Assimilation,
    Revolt,
    PolityFragmented,
    Succession,
    BackgroundEvent,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EventSubject {
    Region(RegionId),
    Settlement(SettlementId),
    PopulationGroup(PopulationGroupId),
    Culture(CultureId),
    Polity(PolityId),
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
        let (settlements, population_groups, cultures) = initialize_living_history(&world, &config);
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
                cultures,
                polities: Vec::new(),
                trade_links: Vec::new(),
                rivalries: Vec::new(),
                alliances: Vec::new(),
                wars: Vec::new(),
                treaties: Vec::new(),
                event_count: 0,
            },
            pending_events: Vec::new(),
        }
    }

    pub fn from_compiled_scenario(compiled: CompiledScenario) -> Self {
        let population = compiled
            .population_groups
            .iter()
            .map(|group| group.population)
            .sum();
        let event_count = compiled.background_events.len() as u64;

        Self {
            config: compiled.config,
            seed: compiled.seed,
            state: SimulationState {
                month: 0,
                world: compiled.world,
                population,
                settlements: compiled.settlements,
                population_groups: compiled.population_groups,
                cultures: compiled.cultures,
                polities: compiled.polities,
                trade_links: Vec::new(),
                rivalries: Vec::new(),
                alliances: Vec::new(),
                wars: Vec::new(),
                treaties: Vec::new(),
                event_count,
            },
            pending_events: compiled.background_events,
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

        if self.config.civilization.enabled {
            self.apply_civilization(&mut events);
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
        let mut events = Vec::new();
        events.extend(self.drain_pending_events());
        events.extend((0..months).flat_map(|_| self.tick_month()));
        events
    }

    pub fn drain_pending_events(&mut self) -> Vec<SimulationEvent> {
        let mut events = Vec::new();
        events.append(&mut self.pending_events);
        events
    }

    pub fn snapshot(&self) -> SimulationSnapshot {
        SimulationSnapshot {
            seed: self.seed,
            state: self.state.clone(),
        }
    }

    pub fn config(&self) -> &SimulationConfig {
        &self.config
    }

    fn apply_civilization(&mut self, events: &mut Vec<SimulationEvent>) {
        self.found_polities(events);
        self.expand_polities(events);
        self.form_trade_links(events);
        self.apply_border_tension(events);
        self.form_alliances(events);
        self.declare_wars(events);
        self.progress_wars(events);
        self.apply_assimilation(events);
        self.apply_fragmentation(events);
        self.apply_succession(events);
        self.apply_cultural_drift(events);
        self.collapse_unstable_polities(events);
    }

    fn found_polities(&mut self, events: &mut Vec<SimulationEvent>) {
        let threshold = self.config.civilization.polity_foundation_population;
        let candidates: Vec<SettlementId> = self
            .state
            .settlements
            .iter()
            .filter(|settlement| {
                settlement.status == SettlementStatus::Active && settlement.polity.is_none()
            })
            .filter(|settlement| !self.settlement_has_polity_history(settlement.id))
            .filter(|settlement| self.state.settlement_population(settlement.id) >= threshold)
            .map(|settlement| settlement.id)
            .collect();

        for settlement_id in candidates {
            let Some(culture_id) = self.primary_culture_for_settlement(settlement_id) else {
                continue;
            };
            let polity_id = self.state.polities.len();
            let settlement = self.state.settlements[settlement_id].clone();
            self.state.settlements[settlement_id].polity = Some(polity_id);
            self.state.polities.push(Polity {
                id: polity_id,
                name: format!("{} Compact", settlement.name),
                status: PolityStatus::Active,
                primary_culture: culture_id,
                capital: settlement_id,
                controlled_settlements: vec![settlement_id],
                controlled_regions: vec![settlement.region],
                institutions: institutions_for(&settlement, &self.state.world),
                succession_count: 0,
                parent: None,
                cohesion: 100,
            });
            events.push(self.make_event(
                EventType::PolityFounded,
                EventSeverity::Important,
                vec!["polity".to_string(), "institution".to_string()],
                vec![
                    EventSubject::Polity(polity_id),
                    EventSubject::Settlement(settlement_id),
                    EventSubject::Culture(culture_id),
                    EventSubject::Region(settlement.region),
                ],
                vec![format!(
                    "{} reached {} people",
                    settlement.name,
                    self.state.settlement_population(settlement_id)
                )],
                vec![format!(
                    "{} formed around {}",
                    self.state.polities[polity_id].name, settlement.name
                )],
                format!(
                    "{} formed as households in {} accepted shared institutions.",
                    self.state.polities[polity_id].name, settlement.name
                ),
            ));
        }
    }

    fn expand_polities(&mut self, events: &mut Vec<SimulationEvent>) {
        let threshold = self
            .config
            .civilization
            .expansion_pressure_threshold_per_mille as u64;
        let mut expansions = Vec::new();
        for polity in self
            .state
            .polities
            .iter()
            .filter(|polity| polity.status == PolityStatus::Active)
        {
            for settlement in self.state.settlements.iter().filter(|settlement| {
                settlement.status == SettlementStatus::Active && settlement.polity.is_none()
            }) {
                let borders = polity.controlled_regions.iter().any(|region| {
                    self.state.world.regions[*region]
                        .neighbors
                        .contains(&settlement.region)
                });
                if borders && self.pressure_per_mille(polity.capital) >= threshold {
                    expansions.push((polity.id, settlement.id));
                }
            }
        }

        for (polity_id, settlement_id) in expansions {
            if self.state.settlements[settlement_id].polity.is_some()
                || self.state.polities[polity_id].status != PolityStatus::Active
            {
                continue;
            }
            self.state.settlements[settlement_id].polity = Some(polity_id);
            let region = self.state.settlements[settlement_id].region;
            let polity = &mut self.state.polities[polity_id];
            polity.controlled_settlements.push(settlement_id);
            if !polity.controlled_regions.contains(&region) {
                polity.controlled_regions.push(region);
                polity.controlled_regions.sort_unstable();
            }
            let polity_name = polity.name.clone();
            let settlement_name = self.state.settlements[settlement_id].name.clone();
            events.push(self.make_event(
                EventType::PolityExpanded,
                EventSeverity::Important,
                vec!["polity".to_string(), "border".to_string()],
                vec![
                    EventSubject::Polity(polity_id),
                    EventSubject::Settlement(settlement_id),
                    EventSubject::Region(region),
                ],
                vec!["neighboring settlement was exposed to expansion pressure".to_string()],
                vec![format!("{polity_name} claimed {settlement_name}")],
                format!("{polity_name} expanded its institutions into {settlement_name}."),
            ));
        }
    }

    fn form_trade_links(&mut self, events: &mut Vec<SimulationEvent>) {
        let interval = self.config.civilization.trade_interval_months.max(1);
        if !self.state.month.is_multiple_of(interval) {
            return;
        }
        for (left, right) in self.neighboring_active_polity_pairs() {
            if self.trade_link_exists(left, right) {
                continue;
            }
            let resources = self.complementary_resources(left, right);
            if resources.is_empty() {
                continue;
            }
            let id = self.state.trade_links.len();
            self.state.trade_links.push(TradeLink {
                id,
                polities: ordered_pair(left, right),
                resources: resources.clone(),
                strength: 25,
                founded_month: self.state.month,
            });
            events.push(self.make_event(
                EventType::TradeLinkFormed,
                EventSeverity::Important,
                vec!["trade".to_string(), "polity".to_string()],
                vec![EventSubject::Polity(left), EventSubject::Polity(right)],
                vec!["neighboring polities held complementary resources".to_string()],
                vec![format!("trade linked resources {:?}", resources)],
                format!(
                    "{} and {} opened a trade link.",
                    self.state.polities[left].name, self.state.polities[right].name
                ),
            ));
        }
    }

    fn apply_border_tension(&mut self, events: &mut Vec<SimulationEvent>) {
        let interval = self.config.civilization.tension_interval_months.max(1);
        if !self.state.month.is_multiple_of(interval) {
            return;
        }
        for (left, right) in self.neighboring_active_polity_pairs() {
            let Some(left_region) = self.representative_region(left) else {
                continue;
            };
            let Some(right_region) = self.representative_region(right) else {
                continue;
            };
            let pressure = self.pressure_per_mille(self.state.polities[left].capital)
                + self.pressure_per_mille(self.state.polities[right].capital);
            if pressure < 2_000 && self.trade_link_exists(left, right) {
                continue;
            }
            let rivalry_id = self.record_rivalry(left, right, 15);
            self.state.polities[left].cohesion -= 10;
            self.state.polities[right].cohesion -= 10;
            events.push(self.make_event(
                EventType::BorderTension,
                EventSeverity::Important,
                vec![
                    "border".to_string(),
                    "tension".to_string(),
                    "polity".to_string(),
                ],
                vec![
                    EventSubject::Polity(left),
                    EventSubject::Polity(right),
                    EventSubject::Region(left_region),
                    EventSubject::Region(right_region),
                ],
                vec![format!(
                    "border pressure reached {pressure} combined per mille"
                )],
                vec![format!("rivalry {rivalry_id} intensified")],
                format!(
                    "Border tension rose between {} and {}.",
                    self.state.polities[left].name, self.state.polities[right].name
                ),
            ));
        }
    }

    fn form_alliances(&mut self, events: &mut Vec<SimulationEvent>) {
        let interval = self.config.civilization.alliance_interval_months.max(1);
        if !self.state.month.is_multiple_of(interval) {
            return;
        }
        for (left, right) in self.neighboring_active_polity_pairs() {
            if self.alliance_exists(left, right) || self.active_war_exists(left, right) {
                continue;
            }
            if !self.trade_link_exists(left, right) && self.rivalry_tension(left, right) > 0 {
                continue;
            }
            let cause = if self.trade_link_exists(left, right) {
                "trade and low rivalry made cooperation useful"
            } else {
                "low rivalry made border cooperation useful"
            };
            let id = self.state.alliances.len();
            self.state.alliances.push(Alliance {
                id,
                polities: ordered_pair(left, right),
                status: AllianceStatus::Active,
                formed_month: self.state.month,
            });
            events.push(self.make_event(
                EventType::AllianceFormed,
                EventSeverity::Important,
                vec!["alliance".to_string(), "polity".to_string()],
                vec![EventSubject::Polity(left), EventSubject::Polity(right)],
                vec![cause.to_string()],
                vec!["rivalry tension softened".to_string()],
                format!(
                    "{} and {} formed an alliance.",
                    self.state.polities[left].name, self.state.polities[right].name
                ),
            ));
        }
    }

    fn declare_wars(&mut self, events: &mut Vec<SimulationEvent>) {
        let threshold = self.config.civilization.war_tension_threshold;
        for (left, right) in self.neighboring_active_polity_pairs() {
            let tension = self.rivalry_tension(left, right);
            if tension < threshold
                || self.active_alliance_exists(left, right)
                || self.active_war_exists(left, right)
            {
                continue;
            }
            self.break_alliance(left, right);
            let id = self.state.wars.len();
            self.state.wars.push(War {
                id,
                polities: ordered_pair(left, right),
                status: WarStatus::Active,
                started_month: self.state.month,
                ended_month: None,
                tension_at_start: tension,
                score: 0,
            });
            events.push(self.make_event(
                EventType::WarDeclared,
                EventSeverity::Important,
                vec!["war".to_string(), "rivalry".to_string()],
                vec![EventSubject::Polity(left), EventSubject::Polity(right)],
                vec![format!("rivalry tension reached {tension}")],
                vec!["war began between neighboring polities".to_string()],
                format!(
                    "{} and {} went to war after rivalry hardened.",
                    self.state.polities[left].name, self.state.polities[right].name
                ),
            ));
        }
    }

    fn progress_wars(&mut self, events: &mut Vec<SimulationEvent>) {
        let interval = self.config.civilization.war_interval_months.max(1);
        if !self.state.month.is_multiple_of(interval) {
            return;
        }
        let active_wars = self
            .state
            .wars
            .iter()
            .filter(|war| war.status == WarStatus::Active)
            .map(|war| war.id)
            .collect::<Vec<_>>();
        for war_id in active_wars {
            let (left, right) = self.state.wars[war_id].polities;
            if self.state.polities[left].status != PolityStatus::Active
                || self.state.polities[right].status != PolityStatus::Active
            {
                self.end_war_after_collapse(war_id, events);
                continue;
            }
            let pressure = self.pressure_per_mille(self.state.polities[left].capital)
                + self.pressure_per_mille(self.state.polities[right].capital);
            let increment = 10 + (pressure / 1_000) as i32;
            self.state.wars[war_id].score += increment;
            self.state.polities[left].cohesion -= 5;
            self.state.polities[right].cohesion -= 5;
            if self.state.wars[war_id].score >= self.config.civilization.war_end_score_threshold {
                self.end_war_with_treaty(war_id, events);
            }
        }
    }

    fn end_war_with_treaty(&mut self, war_id: WarId, events: &mut Vec<SimulationEvent>) {
        let (left, right) = self.state.wars[war_id].polities;
        self.state.wars[war_id].status = WarStatus::Ended;
        self.state.wars[war_id].ended_month = Some(self.state.month);
        let treaty_id = self.state.treaties.len();
        let mut terms = vec![TreatyTerm::Truce, TreatyTerm::Recognition];
        if self.trade_link_exists(left, right) {
            terms.push(TreatyTerm::TradeAccess);
        }
        self.state.treaties.push(Treaty {
            id: treaty_id,
            polities: ordered_pair(left, right),
            war: Some(war_id),
            terms: terms.clone(),
            signed_month: self.state.month,
        });
        events.push(self.make_event(
            EventType::WarEnded,
            EventSeverity::Important,
            vec!["war".to_string(), "treaty".to_string()],
            vec![EventSubject::Polity(left), EventSubject::Polity(right)],
            vec![format!(
                "war score reached {}",
                self.state.wars[war_id].score
            )],
            vec!["war ended in negotiated settlement".to_string()],
            format!(
                "{} and {} ended their war.",
                self.state.polities[left].name, self.state.polities[right].name
            ),
        ));
        events.push(self.make_event(
            EventType::TreatySigned,
            EventSeverity::Important,
            vec!["treaty".to_string(), "war".to_string()],
            vec![EventSubject::Polity(left), EventSubject::Polity(right)],
            vec!["war exhaustion forced negotiation".to_string()],
            vec![format!("treaty terms recorded as {:?}", terms)],
            format!(
                "{} and {} signed a treaty.",
                self.state.polities[left].name, self.state.polities[right].name
            ),
        ));
    }

    fn end_war_after_collapse(&mut self, war_id: WarId, events: &mut Vec<SimulationEvent>) {
        let (left, right) = self.state.wars[war_id].polities;
        self.state.wars[war_id].status = WarStatus::Ended;
        self.state.wars[war_id].ended_month = Some(self.state.month);
        events.push(self.make_event(
            EventType::WarEnded,
            EventSeverity::Important,
            vec!["war".to_string(), "collapse".to_string()],
            vec![EventSubject::Polity(left), EventSubject::Polity(right)],
            vec!["one side could no longer sustain active institutions".to_string()],
            vec!["war ended without treaty after polity collapse".to_string()],
            format!(
                "The war between {} and {} ended after collapse broke the conflict.",
                self.state.polities[left].name, self.state.polities[right].name
            ),
        ));
    }

    fn apply_assimilation(&mut self, events: &mut Vec<SimulationEvent>) {
        let interval = self.config.civilization.assimilation_interval_months.max(1);
        if !self.state.month.is_multiple_of(interval) {
            return;
        }
        let mut assimilations = Vec::new();
        for polity in self
            .state
            .polities
            .iter()
            .filter(|polity| polity.status == PolityStatus::Active)
        {
            for group in self.state.population_groups.iter().filter(|group| {
                group.settlement.is_some_and(|settlement_id| {
                    self.state.settlements[settlement_id].polity == Some(polity.id)
                }) && group.culture.is_some()
                    && group.culture != Some(polity.primary_culture)
            }) {
                assimilations.push((polity.id, group.id, group.culture.expect("culture")));
            }
        }
        for (polity_id, group_id, old_culture) in assimilations {
            if self.state.population_groups[group_id].culture == Some(old_culture) {
                self.state.population_groups[group_id].culture =
                    Some(self.state.polities[polity_id].primary_culture);
                events.push(self.make_event(
                    EventType::Assimilation,
                    EventSeverity::Important,
                    vec!["culture".to_string(), "assimilation".to_string()],
                    vec![
                        EventSubject::Polity(polity_id),
                        EventSubject::PopulationGroup(group_id),
                        EventSubject::Culture(old_culture),
                        EventSubject::Culture(self.state.polities[polity_id].primary_culture),
                    ],
                    vec!["minority households lived under another polity".to_string()],
                    vec!["population identity shifted without changing population".to_string()],
                    format!(
                        "{} adopted the dominant culture of {}.",
                        self.state.population_groups[group_id].name,
                        self.state.polities[polity_id].name
                    ),
                ));
            }
        }
    }

    fn apply_fragmentation(&mut self, events: &mut Vec<SimulationEvent>) {
        let interval = self
            .config
            .civilization
            .fragmentation_interval_months
            .max(1);
        if !self.state.month.is_multiple_of(interval) {
            return;
        }
        let threshold = self.config.civilization.fragmentation_cohesion_threshold;
        let candidates = self
            .state
            .polities
            .iter()
            .filter(|polity| {
                polity.status == PolityStatus::Active
                    && polity.cohesion <= threshold
                    && polity.controlled_settlements.len() > 1
            })
            .map(|polity| polity.id)
            .collect::<Vec<_>>();
        for parent_id in candidates {
            let Some(&settlement_id) = self.state.polities[parent_id]
                .controlled_settlements
                .iter()
                .find(|settlement_id| **settlement_id != self.state.polities[parent_id].capital)
            else {
                continue;
            };
            let child_id = self.state.polities.len();
            let region = self.state.settlements[settlement_id].region;
            let child_culture = self
                .primary_culture_for_settlement(settlement_id)
                .unwrap_or(self.state.polities[parent_id].primary_culture);
            self.state.settlements[settlement_id].polity = Some(child_id);
            self.sync_polity_holdings_after_settlement_loss(parent_id);
            self.state.polities.push(Polity {
                id: child_id,
                name: format!(
                    "{} Free Compact",
                    self.state.settlements[settlement_id].name
                ),
                status: PolityStatus::Active,
                primary_culture: child_culture,
                capital: settlement_id,
                controlled_settlements: vec![settlement_id],
                controlled_regions: vec![region],
                institutions: vec![Institution::Council],
                succession_count: 0,
                parent: Some(parent_id),
                cohesion: 120,
            });
            events.push(self.make_event(
                EventType::Revolt,
                EventSeverity::Important,
                vec!["revolt".to_string(), "fragmentation".to_string()],
                vec![
                    EventSubject::Polity(parent_id),
                    EventSubject::Polity(child_id),
                    EventSubject::Settlement(settlement_id),
                ],
                vec![format!(
                    "cohesion fell to {}",
                    self.state.polities[parent_id].cohesion
                )],
                vec!["a border settlement rejected central authority".to_string()],
                format!(
                    "{} revolted against {}.",
                    self.state.settlements[settlement_id].name, self.state.polities[parent_id].name
                ),
            ));
            events.push(self.make_event(
                EventType::PolityFragmented,
                EventSeverity::Important,
                vec!["polity".to_string(), "fragmentation".to_string()],
                vec![
                    EventSubject::Polity(parent_id),
                    EventSubject::Polity(child_id),
                    EventSubject::Region(region),
                ],
                vec!["revolt created a child polity".to_string()],
                vec![format!(
                    "{} became independent",
                    self.state.polities[child_id].name
                )],
                format!(
                    "{} fragmented from {}.",
                    self.state.polities[child_id].name, self.state.polities[parent_id].name
                ),
            ));
        }
    }

    fn apply_succession(&mut self, events: &mut Vec<SimulationEvent>) {
        let interval = self.config.civilization.succession_interval_months.max(1);
        if !self.state.month.is_multiple_of(interval) {
            return;
        }
        let candidates = self
            .state
            .polities
            .iter()
            .filter(|polity| polity.status == PolityStatus::Active)
            .map(|polity| polity.id)
            .collect::<Vec<_>>();
        for polity_id in candidates {
            self.state.polities[polity_id].succession_count += 1;
            if self.state.polities[polity_id]
                .institutions
                .contains(&Institution::TradeLeague)
            {
                self.state.polities[polity_id].cohesion += 2;
            } else {
                self.state.polities[polity_id].cohesion -= 5;
            }
            events.push(self.make_event(
                EventType::Succession,
                EventSeverity::Important,
                vec!["succession".to_string(), "institution".to_string()],
                vec![EventSubject::Polity(polity_id)],
                vec!["institutional leadership changed".to_string()],
                vec![format!(
                    "succession count reached {}",
                    self.state.polities[polity_id].succession_count
                )],
                format!(
                    "{} passed through a succession.",
                    self.state.polities[polity_id].name
                ),
            ));
        }
    }

    fn apply_cultural_drift(&mut self, events: &mut Vec<SimulationEvent>) {
        let interval = self
            .config
            .civilization
            .cultural_drift_interval_months
            .max(1);
        if !self.state.month.is_multiple_of(interval) {
            return;
        }
        for culture_id in 0..self.state.cultures.len() {
            let mut regions: Vec<RegionId> = self
                .state
                .population_groups
                .iter()
                .filter(|group| group.culture == Some(culture_id))
                .map(|group| group.region)
                .collect();
            regions.sort_unstable();
            regions.dedup();
            if regions.len() < 2 {
                continue;
            }
            self.state.cultures[culture_id].drift += regions.len() as i32;
            let culture_name = self.state.cultures[culture_id].name.clone();
            events.push(self.make_event(
                EventType::CultureDrift,
                EventSeverity::Important,
                vec!["culture".to_string(), "drift".to_string()],
                vec![EventSubject::Culture(culture_id)],
                vec![format!("culture spanned {} regions", regions.len())],
                vec![format!("{culture_name} drift increased")],
                format!("{culture_name} changed as its households spread between regions."),
            ));
        }
    }

    fn collapse_unstable_polities(&mut self, events: &mut Vec<SimulationEvent>) {
        let threshold = self.config.civilization.collapse_cohesion_threshold;
        let candidates: Vec<PolityId> = self
            .state
            .polities
            .iter()
            .filter(|polity| polity.status == PolityStatus::Active && polity.cohesion <= threshold)
            .map(|polity| polity.id)
            .collect();
        for polity_id in candidates {
            let controlled = self.state.polities[polity_id]
                .controlled_settlements
                .clone();
            for settlement_id in controlled {
                if self.state.settlements[settlement_id].polity == Some(polity_id) {
                    self.state.settlements[settlement_id].polity = None;
                }
            }
            self.state.polities[polity_id].status = PolityStatus::Collapsed;
            let polity_name = self.state.polities[polity_id].name.clone();
            events.push(self.make_event(
                EventType::PolityCollapse,
                EventSeverity::Important,
                vec!["polity".to_string(), "collapse".to_string()],
                vec![EventSubject::Polity(polity_id)],
                vec![format!(
                    "cohesion fell to {}",
                    self.state.polities[polity_id].cohesion
                )],
                vec![format!("{polity_name} released its settlements")],
                format!("{polity_name} collapsed after its cohesion failed."),
            ));
        }
    }

    fn primary_culture_for_settlement(&self, settlement_id: SettlementId) -> Option<CultureId> {
        self.state
            .population_groups
            .iter()
            .find(|group| group.settlement == Some(settlement_id))
            .and_then(|group| group.culture)
    }

    fn settlement_has_polity_history(&self, settlement_id: SettlementId) -> bool {
        self.state
            .polities
            .iter()
            .any(|polity| polity.controlled_settlements.contains(&settlement_id))
    }

    fn representative_region(&self, polity_id: PolityId) -> Option<RegionId> {
        self.state.polities[polity_id]
            .controlled_regions
            .first()
            .copied()
            .or_else(|| {
                self.state.polities[polity_id]
                    .controlled_settlements
                    .first()
                    .map(|settlement_id| self.state.settlements[*settlement_id].region)
            })
    }

    fn sync_polity_holdings_after_settlement_loss(&mut self, polity_id: PolityId) {
        let controlled_settlements = self.state.polities[polity_id]
            .controlled_settlements
            .iter()
            .copied()
            .filter(|settlement_id| {
                self.state
                    .settlements
                    .get(*settlement_id)
                    .is_some_and(|settlement| {
                        settlement.polity == Some(polity_id)
                            && settlement.status != SettlementStatus::Abandoned
                    })
            })
            .collect::<Vec<_>>();

        let mut controlled_regions = controlled_settlements
            .iter()
            .map(|settlement_id| self.state.settlements[*settlement_id].region)
            .collect::<Vec<_>>();
        controlled_regions.sort_unstable();
        controlled_regions.dedup();

        let polity = &mut self.state.polities[polity_id];
        polity.controlled_settlements = controlled_settlements;
        polity.controlled_regions = controlled_regions;
        if polity.controlled_settlements.is_empty() {
            polity.status = PolityStatus::Collapsed;
        }
    }

    fn neighboring_active_polity_pairs(&self) -> Vec<(PolityId, PolityId)> {
        let mut pairs = Vec::new();
        for left in self
            .state
            .polities
            .iter()
            .filter(|polity| polity.status == PolityStatus::Active)
        {
            for right in self
                .state
                .polities
                .iter()
                .filter(|polity| polity.status == PolityStatus::Active && polity.id > left.id)
            {
                let neighbors = left.controlled_regions.iter().any(|left_region| {
                    right.controlled_regions.iter().any(|right_region| {
                        self.state.world.regions[*left_region]
                            .neighbors
                            .contains(right_region)
                    })
                });
                if neighbors {
                    pairs.push((left.id, right.id));
                }
            }
        }
        pairs
    }

    fn trade_link_exists(&self, left: PolityId, right: PolityId) -> bool {
        let pair = ordered_pair(left, right);
        self.state
            .trade_links
            .iter()
            .any(|link| link.polities == pair)
    }

    fn alliance_exists(&self, left: PolityId, right: PolityId) -> bool {
        let pair = ordered_pair(left, right);
        self.state
            .alliances
            .iter()
            .any(|alliance| alliance.polities == pair)
    }

    fn active_alliance_exists(&self, left: PolityId, right: PolityId) -> bool {
        let pair = ordered_pair(left, right);
        self.state
            .alliances
            .iter()
            .any(|alliance| alliance.polities == pair && alliance.status == AllianceStatus::Active)
    }

    fn active_war_exists(&self, left: PolityId, right: PolityId) -> bool {
        let pair = ordered_pair(left, right);
        self.state
            .wars
            .iter()
            .any(|war| war.polities == pair && war.status == WarStatus::Active)
    }

    fn break_alliance(&mut self, left: PolityId, right: PolityId) {
        let pair = ordered_pair(left, right);
        for alliance in self
            .state
            .alliances
            .iter_mut()
            .filter(|alliance| alliance.polities == pair)
        {
            alliance.status = AllianceStatus::Broken;
        }
    }

    fn rivalry_tension(&self, left: PolityId, right: PolityId) -> i32 {
        let pair = ordered_pair(left, right);
        self.state
            .rivalries
            .iter()
            .find(|rivalry| rivalry.polities == pair)
            .map(|rivalry| rivalry.tension)
            .unwrap_or(0)
    }

    fn complementary_resources(&self, left: PolityId, right: PolityId) -> Vec<Resource> {
        let left_resources = self.resources_for_polity(left);
        let right_resources = self.resources_for_polity(right);
        let mut resources: Vec<Resource> = left_resources
            .iter()
            .filter(|resource| !right_resources.contains(resource))
            .chain(
                right_resources
                    .iter()
                    .filter(|resource| !left_resources.contains(resource)),
            )
            .cloned()
            .collect();
        resources.sort_by_key(|resource| format!("{resource:?}"));
        resources.dedup();
        resources
    }

    fn resources_for_polity(&self, polity_id: PolityId) -> Vec<Resource> {
        let mut resources = Vec::new();
        for region_id in &self.state.polities[polity_id].controlled_regions {
            resources.extend(
                self.state.world.regions[*region_id]
                    .resources
                    .iter()
                    .cloned(),
            );
        }
        resources.sort_by_key(|resource| format!("{resource:?}"));
        resources.dedup();
        resources
    }

    fn record_rivalry(&mut self, left: PolityId, right: PolityId, added_tension: i32) -> RivalryId {
        let pair = ordered_pair(left, right);
        if let Some(rivalry) = self
            .state
            .rivalries
            .iter_mut()
            .find(|rivalry| rivalry.polities == pair)
        {
            rivalry.tension += added_tension;
            return rivalry.id;
        }

        let id = self.state.rivalries.len();
        self.state.rivalries.push(Rivalry {
            id,
            polities: pair,
            tension: added_tension,
            started_month: self.state.month,
        });
        id
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
        if !self.state.month.is_multiple_of(6) {
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
            polity: None,
        });

        let group_id = self.state.population_groups.len();
        self.state.population_groups.push(PopulationGroup {
            id: group_id,
            name: format!("{} migrants", self.state.world.regions[target_region].name),
            region: target_region,
            settlement: Some(settlement_id),
            population: migrants,
            subsistence: SubsistenceMode::Farming,
            culture: self.state.population_groups[origin_group_id].culture,
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
        let polity_id = settlement.polity.take();
        let settlement = settlement.clone();

        if let Some(polity_id) = polity_id {
            self.sync_polity_holdings_after_settlement_loss(polity_id);
            let polity = &mut self.state.polities[polity_id];
            polity.cohesion -= 20;
        }

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

    #[allow(clippy::too_many_arguments)]
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

pub(crate) fn initialize_living_history(
    world: &World,
    config: &SimulationConfig,
) -> (Vec<Settlement>, Vec<PopulationGroup>, Vec<Culture>) {
    let settings = &config.living_history;
    let settlement_count = settings.initial_settlements.max(1).min(world.regions.len());
    let mut settlements = Vec::with_capacity(settlement_count);
    let mut population_groups = Vec::with_capacity(settlement_count);
    let mut cultures = Vec::with_capacity(settlement_count);

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
            polity: None,
        });
        population_groups.push(PopulationGroup {
            id: settlement_id,
            name: format!("{} households", region.name),
            region: region.id,
            settlement: Some(settlement_id),
            population,
            subsistence: SubsistenceMode::Farming,
            culture: config.civilization.enabled.then_some(settlement_id),
        });
        if config.civilization.enabled {
            cultures.push(Culture {
                id: settlement_id,
                name: format!("{} Folk", region.name),
                origin_region: region.id,
                traits: traits_for(region),
                naming: naming_for(region),
                drift: 0,
            });
        }
    }

    (settlements, population_groups, cultures)
}

fn ordered_pair(left: PolityId, right: PolityId) -> (PolityId, PolityId) {
    if left < right {
        (left, right)
    } else {
        (right, left)
    }
}

fn traits_for(region: &crate::world::Region) -> Vec<CultureTrait> {
    let biome_trait = match region.biome {
        Biome::Tundra => CultureTrait::Highland,
        Biome::Forest | Biome::Rainforest => CultureTrait::Forest,
        Biome::Grassland => CultureTrait::Steppe,
        Biome::Desert => CultureTrait::Mercantile,
    };
    let resource_trait = if region.resources.contains(&Resource::Fish) {
        CultureTrait::Maritime
    } else if region.resources.contains(&Resource::Salt)
        || region.resources.contains(&Resource::Copper)
    {
        CultureTrait::Mercantile
    } else {
        CultureTrait::Riverine
    };
    vec![biome_trait, resource_trait]
}

fn naming_for(region: &crate::world::Region) -> NamingTradition {
    let prefix = region
        .name
        .chars()
        .take(3)
        .collect::<String>()
        .to_ascii_titlecase();
    NamingTradition::PrefixSuffix {
        starts: vec![prefix, "Ana".to_string(), "Bel".to_string()],
        ends: vec!["mar".to_string(), "ven".to_string(), "tor".to_string()],
    }
}

trait AsciiTitlecase {
    fn to_ascii_titlecase(self) -> String;
}

impl AsciiTitlecase for String {
    fn to_ascii_titlecase(mut self) -> String {
        if let Some(first) = self.get_mut(0..1) {
            first.make_ascii_uppercase();
        }
        self
    }
}

fn institutions_for(settlement: &Settlement, world: &World) -> Vec<Institution> {
    let region = &world.regions[settlement.region];
    let mut institutions = vec![Institution::Council];
    if region.resources.contains(&Resource::Copper) || region.resources.contains(&Resource::Horses)
    {
        institutions.push(Institution::MilitaryCommand);
    } else if region.resources.contains(&Resource::Salt)
        || region.resources.contains(&Resource::Fish)
    {
        institutions.push(Institution::TradeLeague);
    } else if matches!(region.biome, Biome::Desert | Biome::Tundra) {
        institutions.push(Institution::Chiefdom);
    } else {
        institutions.push(Institution::TempleAuthority);
    }
    institutions
}

fn initial_population(carrying_capacity: u32, settings: &LivingHistoryConfig) -> u64 {
    ((carrying_capacity as u64 * settings.initial_population_per_mille as u64) / 1_000).max(1)
}

pub(crate) fn seeded_rng(seed: SimulationSeed, stream: u64) -> ChaCha8Rng {
    ChaCha8Rng::seed_from_u64(seed.value() ^ stream.rotate_left(17))
}
