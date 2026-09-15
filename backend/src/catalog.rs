//! Static Valorant reference data: the agent pool (with roles) and the map pool.
//!
//! Kept in code rather than the database so a fresh deployment always knows about
//! every agent and map without needing a migration or an upstream API call.

use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Agent {
    pub name: &'static str,
    pub role: &'static str,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct MapInfo {
    pub name: &'static str,
    /// Whether the map is in the competitive rotation we assume by default.
    pub active: bool,
}

pub const ROLES: [&str; 4] = ["Duelist", "Initiator", "Controller", "Sentinel"];

pub const AGENTS: &[Agent] = &[
    Agent { name: "Jett", role: "Duelist" },
    Agent { name: "Phoenix", role: "Duelist" },
    Agent { name: "Raze", role: "Duelist" },
    Agent { name: "Reyna", role: "Duelist" },
    Agent { name: "Yoru", role: "Duelist" },
    Agent { name: "Neon", role: "Duelist" },
    Agent { name: "Iso", role: "Duelist" },
    Agent { name: "Waylay", role: "Duelist" },
    Agent { name: "Sova", role: "Initiator" },
    Agent { name: "Breach", role: "Initiator" },
    Agent { name: "Skye", role: "Initiator" },
    Agent { name: "KAY/O", role: "Initiator" },
    Agent { name: "Fade", role: "Initiator" },
    Agent { name: "Gekko", role: "Initiator" },
    Agent { name: "Tejo", role: "Initiator" },
    Agent { name: "Brimstone", role: "Controller" },
    Agent { name: "Viper", role: "Controller" },
    Agent { name: "Omen", role: "Controller" },
    Agent { name: "Astra", role: "Controller" },
    Agent { name: "Harbor", role: "Controller" },
    Agent { name: "Clove", role: "Controller" },
    Agent { name: "Killjoy", role: "Sentinel" },
    Agent { name: "Cypher", role: "Sentinel" },
    Agent { name: "Sage", role: "Sentinel" },
    Agent { name: "Chamber", role: "Sentinel" },
    Agent { name: "Deadlock", role: "Sentinel" },
    Agent { name: "Vyse", role: "Sentinel" },
];

pub const MAPS: &[MapInfo] = &[
    MapInfo { name: "Abyss", active: true },
    MapInfo { name: "Ascent", active: true },
    MapInfo { name: "Bind", active: true },
    MapInfo { name: "Breeze", active: false },
    MapInfo { name: "Corrode", active: true },
    MapInfo { name: "Fracture", active: false },
    MapInfo { name: "Haven", active: true },
    MapInfo { name: "Icebox", active: false },
    MapInfo { name: "Lotus", active: true },
    MapInfo { name: "Pearl", active: false },
    MapInfo { name: "Split", active: true },
    MapInfo { name: "Sunset", active: false },
];

/// Role for an agent name, or `"Flex"` when we do not recognise the agent
/// (custom or newly released agents typed in by hand still work everywhere).
pub fn role_of(agent: &str) -> &'static str {
    AGENTS
        .iter()
        .find(|a| a.name.eq_ignore_ascii_case(agent))
        .map(|a| a.role)
        .unwrap_or("Flex")
}
