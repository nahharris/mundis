use serde::{Deserialize, Serialize};

use crate::simulation::{
    EventSeverity, EventSubject, EventType, SettlementStatus, SimulationSnapshot,
};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryQuery {
    pub from_month: Option<u32>,
    pub to_month: Option<u32>,
    pub tag: Option<String>,
    pub event_type: Option<EventType>,
    pub severity: Option<EventSeverity>,
    pub subject: Option<SubjectFilter>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SubjectFilter {
    Region(usize),
    Settlement(usize),
    PopulationGroup(usize),
    Culture(usize),
    Polity(usize),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateAtMonth {
    pub month: u32,
    pub snapshot: SimulationSnapshot,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AtlasState {
    pub month: u32,
    pub population: u64,
    pub regions: Vec<AtlasRegion>,
    pub settlements: Vec<AtlasSettlement>,
    pub polities: Vec<AtlasPolity>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AtlasRegion {
    pub id: usize,
    pub name: String,
    pub climate: String,
    pub biome: String,
    pub carrying_capacity: u32,
    pub population: u64,
    pub neighbors: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AtlasSettlement {
    pub id: usize,
    pub name: String,
    pub region: usize,
    pub population: u64,
    pub polity: Option<usize>,
    pub status: SettlementStatus,
    pub stability: i32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AtlasPolity {
    pub id: usize,
    pub name: String,
    pub capital: usize,
    pub controlled_regions: Vec<usize>,
    pub controlled_settlements: Vec<usize>,
    pub cohesion: i32,
}

impl SubjectFilter {
    pub fn key(self) -> String {
        match self {
            SubjectFilter::Region(id) => format!("region:{id}"),
            SubjectFilter::Settlement(id) => format!("settlement:{id}"),
            SubjectFilter::PopulationGroup(id) => format!("population-group:{id}"),
            SubjectFilter::Culture(id) => format!("culture:{id}"),
            SubjectFilter::Polity(id) => format!("polity:{id}"),
        }
    }
}

impl From<&EventSubject> for SubjectFilter {
    fn from(subject: &EventSubject) -> Self {
        match subject {
            EventSubject::Region(id) => Self::Region(*id),
            EventSubject::Settlement(id) => Self::Settlement(*id),
            EventSubject::PopulationGroup(id) => Self::PopulationGroup(*id),
            EventSubject::Culture(id) => Self::Culture(*id),
            EventSubject::Polity(id) => Self::Polity(*id),
        }
    }
}

pub fn event_type_key(event_type: &EventType) -> String {
    serde_json::to_value(event_type)
        .ok()
        .and_then(|value| value.as_str().map(ToOwned::to_owned))
        .unwrap_or_else(|| format!("{event_type:?}"))
}

pub fn severity_key(severity: &EventSeverity) -> String {
    serde_json::to_value(severity)
        .ok()
        .and_then(|value| value.as_str().map(ToOwned::to_owned))
        .unwrap_or_else(|| format!("{severity:?}"))
}

pub fn atlas_state(snapshot: &SimulationSnapshot) -> AtlasState {
    let state = &snapshot.state;
    AtlasState {
        month: state.month,
        population: state.population,
        regions: state
            .world
            .regions
            .iter()
            .map(|region| AtlasRegion {
                id: region.id,
                name: region.name.clone(),
                climate: format!("{:?}", region.climate),
                biome: format!("{:?}", region.biome),
                carrying_capacity: region.carrying_capacity,
                population: state
                    .population_groups
                    .iter()
                    .filter(|group| group.region == region.id)
                    .map(|group| group.population)
                    .sum(),
                neighbors: region.neighbors.clone(),
            })
            .collect(),
        settlements: state
            .settlements
            .iter()
            .map(|settlement| AtlasSettlement {
                id: settlement.id,
                name: settlement.name.clone(),
                region: settlement.region,
                population: state.settlement_population(settlement.id),
                polity: settlement.polity,
                status: settlement.status.clone(),
                stability: settlement.stability,
            })
            .collect(),
        polities: state
            .polities
            .iter()
            .map(|polity| AtlasPolity {
                id: polity.id,
                name: polity.name.clone(),
                capital: polity.capital,
                controlled_regions: polity.controlled_regions.clone(),
                controlled_settlements: polity.controlled_settlements.clone(),
                cohesion: polity.cohesion,
            })
            .collect(),
    }
}
