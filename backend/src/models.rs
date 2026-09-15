use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Player {
    pub id: i64,
    pub name: String,
    pub riot_id: Option<String>,
    pub role: String,
    pub rank: Option<String>,
    pub active: bool,
    pub created_at: String,
    /// Riot's stable player id. Lets imported matches attach to the right
    /// person even after a display-name change.
    pub puuid: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PlayerInput {
    pub name: String,
    #[serde(default)]
    pub riot_id: Option<String>,
    #[serde(default = "default_role")]
    pub role: String,
    #[serde(default)]
    pub rank: Option<String>,
    #[serde(default = "default_true")]
    pub active: bool,
    #[serde(default)]
    pub puuid: Option<String>,
}

fn default_role() -> String {
    "Flex".to_string()
}
fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct MatchRow {
    pub id: i64,
    pub played_at: String,
    pub map: String,
    pub mode: String,
    pub rounds_won: i64,
    pub rounds_lost: i64,
    pub notes: Option<String>,
    /// 'manual', 'demo', 'import', or the name of the source it was pulled from.
    pub source: String,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct PerformanceRow {
    pub id: i64,
    pub match_id: i64,
    pub player_id: i64,
    pub agent: String,
    pub kills: i64,
    pub deaths: i64,
    pub assists: i64,
    pub acs: i64,
    pub first_bloods: i64,
    pub first_deaths: i64,
    pub plants: i64,
    pub defuses: i64,
}

/// A match with its per-player scoreboard, the shape the UI reads and writes.
#[derive(Debug, Clone, Serialize)]
pub struct MatchDetail {
    #[serde(flatten)]
    pub info: MatchRow,
    pub result: &'static str,
    pub performances: Vec<PerformanceDetail>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PerformanceDetail {
    #[serde(flatten)]
    pub row: PerformanceRow,
    pub player_name: String,
    pub agent_role: &'static str,
}

#[derive(Debug, Deserialize)]
pub struct MatchInput {
    /// ISO-8601 date or datetime, e.g. "2026-09-14" or "2026-09-14T21:30:00".
    pub played_at: String,
    pub map: String,
    #[serde(default = "default_mode")]
    pub mode: String,
    pub rounds_won: i64,
    pub rounds_lost: i64,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub performances: Vec<PerformanceInput>,
}

fn default_mode() -> String {
    "Competitive".to_string()
}

#[derive(Debug, Deserialize)]
pub struct PerformanceInput {
    pub player_id: i64,
    pub agent: String,
    #[serde(default)]
    pub kills: i64,
    #[serde(default)]
    pub deaths: i64,
    #[serde(default)]
    pub assists: i64,
    #[serde(default)]
    pub acs: i64,
    #[serde(default)]
    pub first_bloods: i64,
    #[serde(default)]
    pub first_deaths: i64,
    #[serde(default)]
    pub plants: i64,
    #[serde(default)]
    pub defuses: i64,
}

pub fn result_of(rounds_won: i64, rounds_lost: i64) -> &'static str {
    match rounds_won.cmp(&rounds_lost) {
        std::cmp::Ordering::Greater => "win",
        std::cmp::Ordering::Less => "loss",
        std::cmp::Ordering::Equal => "draw",
    }
}
