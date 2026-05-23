use serde::{Deserialize, Serialize};

use crate::{
    simulation::SettlementId,
    world::{RegionId, Resource},
};

pub type CultureId = usize;
pub type PolityId = usize;
pub type TradeLinkId = usize;
pub type RivalryId = usize;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Culture {
    pub id: CultureId,
    pub name: String,
    pub origin_region: RegionId,
    pub traits: Vec<CultureTrait>,
    pub drift: i32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CultureTrait {
    Riverine,
    Highland,
    Forest,
    Steppe,
    Maritime,
    Mercantile,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Polity {
    pub id: PolityId,
    pub name: String,
    pub status: PolityStatus,
    pub primary_culture: CultureId,
    pub capital: SettlementId,
    pub controlled_settlements: Vec<SettlementId>,
    pub controlled_regions: Vec<RegionId>,
    pub cohesion: i32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PolityStatus {
    Active,
    Collapsed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TradeLink {
    pub id: TradeLinkId,
    pub polities: (PolityId, PolityId),
    pub resources: Vec<Resource>,
    pub strength: i32,
    pub founded_month: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rivalry {
    pub id: RivalryId,
    pub polities: (PolityId, PolityId),
    pub tension: i32,
    pub started_month: u32,
}
