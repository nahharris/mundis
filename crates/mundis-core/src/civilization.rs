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
    pub naming: NamingTradition,
    pub drift: i32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NamingTradition {
    PrefixSuffix {
        starts: Vec<String>,
        ends: Vec<String>,
    },
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
    pub institutions: Vec<Institution>,
    pub succession_count: u32,
    pub parent: Option<PolityId>,
    pub cohesion: i32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Institution {
    Council,
    Chiefdom,
    TempleAuthority,
    MilitaryCommand,
    TradeLeague,
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

pub type AllianceId = usize;
pub type WarId = usize;
pub type TreatyId = usize;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Alliance {
    pub id: AllianceId,
    pub polities: (PolityId, PolityId),
    pub status: AllianceStatus,
    pub formed_month: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AllianceStatus {
    Active,
    Broken,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct War {
    pub id: WarId,
    pub polities: (PolityId, PolityId),
    pub status: WarStatus,
    pub started_month: u32,
    pub ended_month: Option<u32>,
    pub tension_at_start: i32,
    pub score: i32,
    #[serde(default)]
    pub declared_event_id: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WarStatus {
    Active,
    Ended,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Treaty {
    pub id: TreatyId,
    pub polities: (PolityId, PolityId),
    pub war: Option<WarId>,
    pub terms: Vec<TreatyTerm>,
    pub signed_month: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TreatyTerm {
    Truce,
    BorderTransfer,
    TradeAccess,
    Recognition,
}
