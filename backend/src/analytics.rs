//! All derived statistics.
//!
//! The whole match history for a five-stack is small (thousands of rows at the
//! very most), so every endpoint loads the relevant slice once and folds it in
//! memory. That keeps the aggregations readable and lets them share one set of
//! rules — notably the Bayesian smoothing used everywhere a win rate is shown
//! for a small sample.

use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::catalog;
use crate::error::AppResult;
use crate::models::{result_of, MatchRow, PerformanceRow, Player};

/// Strength of the prior used when smoothing a win rate. A 2-1 record shrinks
/// most of the way back to the team's baseline; a 30-10 record barely moves.
pub const PRIOR_WEIGHT: f64 = 4.0;

/// Query filters shared by every stats endpoint.
#[derive(Debug, Default, Deserialize)]
pub struct StatsFilter {
    /// Only include matches played within the last N days.
    pub days: Option<i64>,
    /// Only include a single map.
    pub map: Option<String>,
    /// Only include a single queue ("Competitive", "Premier", ...).
    pub mode: Option<String>,
}

/// The slice of the database an aggregation runs over.
pub struct Dataset {
    pub players: Vec<Player>,
    pub matches: Vec<MatchRow>,
    pub perfs: Vec<PerformanceRow>,
}

impl Dataset {
    pub async fn load(pool: &SqlitePool, filter: &StatsFilter) -> AppResult<Self> {
        let players: Vec<Player> =
            sqlx::query_as("SELECT id, name, riot_id, role, rank, active, created_at, puuid FROM players ORDER BY name")
                .fetch_all(pool)
                .await?;

        let mut sql = String::from(
            "SELECT id, played_at, map, mode, rounds_won, rounds_lost, notes, source FROM matches WHERE 1=1",
        );
        if filter.days.is_some() {
            sql.push_str(" AND played_at >= datetime('now', ?)");
        }
        if filter.map.is_some() {
            sql.push_str(" AND map = ?");
        }
        if filter.mode.is_some() {
            sql.push_str(" AND mode = ?");
        }
        sql.push_str(" ORDER BY played_at ASC, id ASC");

        let mut q = sqlx::query_as::<_, MatchRow>(&sql);
        if let Some(days) = filter.days {
            q = q.bind(format!("-{} days", days.max(0)));
        }
        if let Some(map) = &filter.map {
            q = q.bind(map.clone());
        }
        if let Some(mode) = &filter.mode {
            q = q.bind(mode.clone());
        }
        let matches: Vec<MatchRow> = q.fetch_all(pool).await?;

        let ids: Vec<i64> = matches.iter().map(|m| m.id).collect();
        let perfs = load_performances(pool, &ids).await?;

        Ok(Self { players, matches, perfs })
    }

    pub fn player_name(&self, id: i64) -> String {
        self.players
            .iter()
            .find(|p| p.id == id)
            .map(|p| p.name.clone())
            .unwrap_or_else(|| format!("Player {id}"))
    }

    /// Team-wide smoothed win rate, used as the prior for every sub-sample.
    pub fn baseline_win_rate(&self) -> f64 {
        let decided: Vec<&MatchRow> = self
            .matches
            .iter()
            .filter(|m| m.rounds_won != m.rounds_lost)
            .collect();
        if decided.is_empty() {
            return 0.5;
        }
        let wins = decided.iter().filter(|m| m.rounds_won > m.rounds_lost).count() as f64;
        wins / decided.len() as f64
    }

    fn perfs_by_match(&self) -> HashMap<i64, Vec<&PerformanceRow>> {
        let mut out: HashMap<i64, Vec<&PerformanceRow>> = HashMap::new();
        for p in &self.perfs {
            out.entry(p.match_id).or_default().push(p);
        }
        out
    }
}

pub async fn load_performances(pool: &SqlitePool, match_ids: &[i64]) -> AppResult<Vec<PerformanceRow>> {
    if match_ids.is_empty() {
        return Ok(Vec::new());
    }
    // SQLite has no array binding; build a placeholder list for the id set.
    let placeholders = std::iter::repeat("?")
        .take(match_ids.len())
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "SELECT id, match_id, player_id, agent, kills, deaths, assists, acs, \
         first_bloods, first_deaths, plants, defuses FROM performances \
         WHERE match_id IN ({placeholders}) ORDER BY id"
    );
    let mut q = sqlx::query_as::<_, PerformanceRow>(&sql);
    for id in match_ids {
        q = q.bind(id);
    }
    Ok(q.fetch_all(pool).await?)
}

// ---------------------------------------------------------------------------
// Small numeric helpers
// ---------------------------------------------------------------------------

pub fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

pub fn round3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

fn ratio(num: f64, den: f64) -> f64 {
    if den <= 0.0 {
        0.0
    } else {
        num / den
    }
}

/// Win rate pulled toward `prior` when the sample is thin. This is the number
/// shown to the user: it reads like a win rate and stays honest on small samples.
pub fn smoothed(wins: f64, played: f64, prior: f64) -> f64 {
    (wins + PRIOR_WEIGHT * prior) / (played + PRIOR_WEIGHT)
}

/// Wilson score lower bound (95%), used to *order* "best of" lists.
///
/// Smoothing alone is not enough for ranking: one 2-0 night on an off-agent
/// still outranks a 27-15 record on a main. The lower bound asks "how good is
/// this at worst", so volume earns its place.
pub fn wilson_lower(wins: f64, played: f64) -> f64 {
    if played <= 0.0 {
        return 0.0;
    }
    const Z: f64 = 1.96;
    let p = wins / played;
    let z2n = Z * Z / played;
    let centre = p + z2n / 2.0;
    let margin = Z * (p * (1.0 - p) / played + z2n / (4.0 * played)).sqrt();
    ((centre - margin) / (1.0 + z2n)).max(0.0)
}

/// K/D that degrades gracefully when a player somehow never died.
fn kd(kills: f64, deaths: f64) -> f64 {
    if deaths <= 0.0 {
        kills
    } else {
        kills / deaths
    }
}

#[derive(Debug, Default, Clone)]
struct Tally {
    played: u32,
    wins: u32,
    losses: u32,
    draws: u32,
    rounds_won: i64,
    rounds_lost: i64,
    kills: i64,
    deaths: i64,
    assists: i64,
    acs: i64,
    acs_samples: u32,
    first_bloods: i64,
    first_deaths: i64,
    plants: i64,
    defuses: i64,
    last_played: Option<String>,
}

impl Tally {
    fn add_match(&mut self, m: &MatchRow) {
        self.played += 1;
        match result_of(m.rounds_won, m.rounds_lost) {
            "win" => self.wins += 1,
            "loss" => self.losses += 1,
            _ => self.draws += 1,
        }
        self.rounds_won += m.rounds_won;
        self.rounds_lost += m.rounds_lost;
        if self
            .last_played
            .as_ref()
            .map(|d| m.played_at > *d)
            .unwrap_or(true)
        {
            self.last_played = Some(m.played_at.clone());
        }
    }

    fn add_perf(&mut self, p: &PerformanceRow) {
        self.kills += p.kills;
        self.deaths += p.deaths;
        self.assists += p.assists;
        self.acs += p.acs;
        self.acs_samples += 1;
        self.first_bloods += p.first_bloods;
        self.first_deaths += p.first_deaths;
        self.plants += p.plants;
        self.defuses += p.defuses;
    }

    fn win_rate(&self) -> f64 {
        let decided = self.wins + self.losses;
        ratio(self.wins as f64, decided as f64)
    }

    fn kd(&self) -> f64 {
        kd(self.kills as f64, self.deaths as f64)
    }

    fn avg_acs(&self) -> f64 {
        ratio(self.acs as f64, self.acs_samples as f64)
    }
}

// ---------------------------------------------------------------------------
// Overview
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct FormEntry {
    pub match_id: i64,
    pub played_at: String,
    pub map: String,
    pub result: &'static str,
    pub rounds_won: i64,
    pub rounds_lost: i64,
}

#[derive(Debug, Serialize)]
pub struct TimelinePoint {
    pub match_id: i64,
    pub played_at: String,
    pub map: String,
    pub result: &'static str,
    pub round_diff: i64,
    pub team_kd: f64,
    pub avg_acs: f64,
    /// Smoothed win rate over the trailing ten matches — the "are we cooking?" line.
    pub rolling_win_rate: f64,
}

#[derive(Debug, Serialize)]
pub struct Overview {
    pub matches: u32,
    pub wins: u32,
    pub losses: u32,
    pub draws: u32,
    pub win_rate: f64,
    pub rounds_won: i64,
    pub rounds_lost: i64,
    pub round_win_rate: f64,
    pub team_kd: f64,
    pub avg_acs: f64,
    pub first_blood_rate: f64,
    /// Positive for a win streak, negative for a losing streak.
    pub streak: i32,
    pub maps_played: usize,
    pub agents_played: usize,
    pub roster_size: usize,
    pub form: Vec<FormEntry>,
    pub timeline: Vec<TimelinePoint>,
    pub best_map: Option<MapStat>,
    pub worst_map: Option<MapStat>,
    pub top_agent: Option<AgentStat>,
    pub mvp: Option<PlayerStat>,
}

pub fn overview(ds: &Dataset) -> Overview {
    let by_match = ds.perfs_by_match();
    let mut tally = Tally::default();
    for m in &ds.matches {
        tally.add_match(m);
        for p in by_match.get(&m.id).into_iter().flatten() {
            tally.add_perf(p);
        }
    }

    let mut results: Vec<&'static str> = Vec::with_capacity(ds.matches.len());
    let mut timeline = Vec::with_capacity(ds.matches.len());
    for (i, m) in ds.matches.iter().enumerate() {
        let res = result_of(m.rounds_won, m.rounds_lost);
        results.push(res);
        let perfs = by_match.get(&m.id).cloned().unwrap_or_default();
        let kills: i64 = perfs.iter().map(|p| p.kills).sum();
        let deaths: i64 = perfs.iter().map(|p| p.deaths).sum();
        let acs: i64 = perfs.iter().map(|p| p.acs).sum();

        let window_start = i.saturating_sub(9);
        let window = &results[window_start..=i];
        let w = window.iter().filter(|r| **r == "win").count() as f64;
        let decided = window.iter().filter(|r| **r != "draw").count() as f64;

        timeline.push(TimelinePoint {
            match_id: m.id,
            played_at: m.played_at.clone(),
            map: m.map.clone(),
            result: res,
            round_diff: m.rounds_won - m.rounds_lost,
            team_kd: round2(kd(kills as f64, deaths as f64)),
            avg_acs: round2(ratio(acs as f64, perfs.len() as f64)),
            rolling_win_rate: round3(ratio(w, decided)),
        });
    }

    // Streak runs backwards from the most recent decided match.
    let mut streak = 0i32;
    for res in results.iter().rev() {
        match (*res, streak) {
            ("win", s) if s >= 0 => streak += 1,
            ("loss", s) if s <= 0 => streak -= 1,
            _ => break,
        }
    }

    let form: Vec<FormEntry> = ds
        .matches
        .iter()
        .rev()
        .take(12)
        .map(|m| FormEntry {
            match_id: m.id,
            played_at: m.played_at.clone(),
            map: m.map.clone(),
            result: result_of(m.rounds_won, m.rounds_lost),
            rounds_won: m.rounds_won,
            rounds_lost: m.rounds_lost,
        })
        .collect();

    let map_stats = map_stats(ds);
    let ranked: Vec<&MapStat> = map_stats.iter().filter(|m| m.played >= 3).collect();
    let best_map = ranked
        .iter()
        .max_by(|a, b| a.rating.total_cmp(&b.rating))
        .map(|m| (*m).clone());
    let worst_map = ranked
        .iter()
        .min_by(|a, b| a.rating.total_cmp(&b.rating))
        .map(|m| (*m).clone());

    let agents = agent_stats(ds);
    // Rank by lower bound with a real minimum sample, so "best agent" is a main
    // the stack leans on rather than a lucky handful of games.
    let top_agent = agents
        .iter()
        .filter(|a| a.picks >= 5)
        .max_by(|a, b| {
            wilson_lower(a.wins as f64, a.picks as f64)
                .total_cmp(&wilson_lower(b.wins as f64, b.picks as f64))
        })
        .cloned();

    let players = player_stats(ds);
    let mvp = players
        .iter()
        .filter(|p| p.matches >= 3)
        .max_by(|a, b| a.rating.total_cmp(&b.rating))
        .cloned();

    Overview {
        matches: tally.played,
        wins: tally.wins,
        losses: tally.losses,
        draws: tally.draws,
        win_rate: round3(tally.win_rate()),
        rounds_won: tally.rounds_won,
        rounds_lost: tally.rounds_lost,
        round_win_rate: round3(ratio(
            tally.rounds_won as f64,
            (tally.rounds_won + tally.rounds_lost) as f64,
        )),
        team_kd: round2(tally.kd()),
        avg_acs: round2(tally.avg_acs()),
        first_blood_rate: round3(ratio(
            tally.first_bloods as f64,
            (tally.first_bloods + tally.first_deaths) as f64,
        )),
        streak,
        maps_played: map_stats.len(),
        agents_played: agents.len(),
        roster_size: ds.players.iter().filter(|p| p.active).count(),
        form,
        timeline,
        best_map,
        worst_map,
        top_agent,
        mvp,
    }
}

// ---------------------------------------------------------------------------
// Maps
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct MapStat {
    pub map: String,
    pub active: bool,
    pub played: u32,
    pub wins: u32,
    pub losses: u32,
    pub draws: u32,
    pub win_rate: f64,
    /// Win rate shrunk toward the team baseline — the value to sort on.
    pub rating: f64,
    pub rounds_won: i64,
    pub rounds_lost: i64,
    pub round_win_rate: f64,
    pub avg_round_diff: f64,
    pub team_kd: f64,
    pub avg_acs: f64,
    pub last_played: Option<String>,
    pub top_agents: Vec<AgentPick>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AgentPick {
    pub agent: String,
    pub role: &'static str,
    pub picks: u32,
    pub win_rate: f64,
    pub player: Option<String>,
}

pub fn map_stats(ds: &Dataset) -> Vec<MapStat> {
    let prior = ds.baseline_win_rate();
    let by_match = ds.perfs_by_match();
    let mut tallies: BTreeMap<String, Tally> = BTreeMap::new();
    let mut agent_counts: BTreeMap<String, BTreeMap<String, (u32, u32)>> = BTreeMap::new();

    for m in &ds.matches {
        let t = tallies.entry(m.map.clone()).or_default();
        t.add_match(m);
        let won = m.rounds_won > m.rounds_lost;
        for p in by_match.get(&m.id).into_iter().flatten() {
            t.add_perf(p);
            let entry = agent_counts
                .entry(m.map.clone())
                .or_default()
                .entry(p.agent.clone())
                .or_default();
            entry.0 += 1;
            if won {
                entry.1 += 1;
            }
        }
    }

    let mut out: Vec<MapStat> = tallies
        .into_iter()
        .map(|(map, t)| {
            let mut top: Vec<AgentPick> = agent_counts
                .get(&map)
                .map(|m| {
                    m.iter()
                        .map(|(agent, (picks, wins))| AgentPick {
                            agent: agent.clone(),
                            role: catalog::role_of(agent),
                            picks: *picks,
                            win_rate: round3(ratio(*wins as f64, *picks as f64)),
                            player: None,
                        })
                        .collect()
                })
                .unwrap_or_default();
            top.sort_by(|a, b| b.picks.cmp(&a.picks).then(a.agent.cmp(&b.agent)));
            top.truncate(6);

            let active = catalog::MAPS
                .iter()
                .find(|mi| mi.name.eq_ignore_ascii_case(&map))
                .map(|mi| mi.active)
                .unwrap_or(true);

            MapStat {
                map,
                active,
                played: t.played,
                wins: t.wins,
                losses: t.losses,
                draws: t.draws,
                win_rate: round3(t.win_rate()),
                rating: round3(smoothed(t.wins as f64, (t.wins + t.losses) as f64, prior)),
                rounds_won: t.rounds_won,
                rounds_lost: t.rounds_lost,
                round_win_rate: round3(ratio(
                    t.rounds_won as f64,
                    (t.rounds_won + t.rounds_lost) as f64,
                )),
                avg_round_diff: round2(ratio(
                    (t.rounds_won - t.rounds_lost) as f64,
                    t.played as f64,
                )),
                team_kd: round2(t.kd()),
                avg_acs: round2(t.avg_acs()),
                last_played: t.last_played.clone(),
                top_agents: top,
            }
        })
        .collect();

    out.sort_by(|a, b| b.rating.total_cmp(&a.rating).then(a.map.cmp(&b.map)));
    out
}

// ---------------------------------------------------------------------------
// Agents
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct AgentStat {
    pub agent: String,
    pub role: &'static str,
    pub picks: u32,
    pub wins: u32,
    pub win_rate: f64,
    pub rating: f64,
    pub kd: f64,
    pub avg_acs: f64,
    pub avg_kills: f64,
    pub first_blood_rate: f64,
    pub players: Vec<AgentPlayerStat>,
    pub maps: Vec<AgentMapStat>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AgentPlayerStat {
    pub agent: String,
    pub player_id: i64,
    pub player: String,
    pub picks: u32,
    pub win_rate: f64,
    pub rating: f64,
    pub kd: f64,
    pub avg_acs: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct AgentMapStat {
    pub map: String,
    pub picks: u32,
    pub win_rate: f64,
}

pub fn agent_stats(ds: &Dataset) -> Vec<AgentStat> {
    let prior = ds.baseline_win_rate();
    let match_by_id: HashMap<i64, &MatchRow> = ds.matches.iter().map(|m| (m.id, m)).collect();

    let mut agents: BTreeMap<String, Tally> = BTreeMap::new();
    let mut per_player: BTreeMap<(String, i64), Tally> = BTreeMap::new();
    let mut per_map: BTreeMap<(String, String), (u32, u32)> = BTreeMap::new();

    for p in &ds.perfs {
        let Some(m) = match_by_id.get(&p.match_id) else {
            continue;
        };
        let t = agents.entry(p.agent.clone()).or_default();
        t.add_match(m);
        t.add_perf(p);

        let pt = per_player.entry((p.agent.clone(), p.player_id)).or_default();
        pt.add_match(m);
        pt.add_perf(p);

        let e = per_map.entry((p.agent.clone(), m.map.clone())).or_default();
        e.0 += 1;
        if m.rounds_won > m.rounds_lost {
            e.1 += 1;
        }
    }

    let mut out: Vec<AgentStat> = agents
        .iter()
        .map(|(agent, t)| {
            let mut players: Vec<AgentPlayerStat> = per_player
                .iter()
                .filter(|((a, _), _)| a == agent)
                .map(|((_, pid), pt)| AgentPlayerStat {
                    agent: agent.clone(),
                    player_id: *pid,
                    player: ds.player_name(*pid),
                    picks: pt.played,
                    win_rate: round3(pt.win_rate()),
                    rating: round3(smoothed(pt.wins as f64, (pt.wins + pt.losses) as f64, prior)),
                    kd: round2(pt.kd()),
                    avg_acs: round2(pt.avg_acs()),
                })
                .collect();
            players.sort_by(|a, b| b.picks.cmp(&a.picks).then(b.rating.total_cmp(&a.rating)));

            let mut maps: Vec<AgentMapStat> = per_map
                .iter()
                .filter(|((a, _), _)| a == agent)
                .map(|((_, map), (picks, wins))| AgentMapStat {
                    map: map.clone(),
                    picks: *picks,
                    win_rate: round3(ratio(*wins as f64, *picks as f64)),
                })
                .collect();
            maps.sort_by(|a, b| b.picks.cmp(&a.picks).then(a.map.cmp(&b.map)));

            AgentStat {
                agent: agent.clone(),
                role: catalog::role_of(agent),
                picks: t.played,
                wins: t.wins,
                win_rate: round3(t.win_rate()),
                rating: round3(smoothed(t.wins as f64, (t.wins + t.losses) as f64, prior)),
                kd: round2(t.kd()),
                avg_acs: round2(t.avg_acs()),
                avg_kills: round2(ratio(t.kills as f64, t.played as f64)),
                first_blood_rate: round3(ratio(
                    t.first_bloods as f64,
                    (t.first_bloods + t.first_deaths) as f64,
                )),
                players,
                maps,
            }
        })
        .collect();

    out.sort_by(|a, b| b.picks.cmp(&a.picks).then(b.rating.total_cmp(&a.rating)));
    out
}

// ---------------------------------------------------------------------------
// Players
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct PlayerStat {
    pub player_id: i64,
    pub name: String,
    pub riot_id: Option<String>,
    pub role: String,
    pub rank: Option<String>,
    pub active: bool,
    pub matches: u32,
    pub wins: u32,
    pub win_rate: f64,
    pub kills: i64,
    pub deaths: i64,
    pub assists: i64,
    pub kd: f64,
    pub kda: f64,
    pub avg_kills: f64,
    pub avg_deaths: f64,
    pub avg_acs: f64,
    pub first_bloods: i64,
    pub first_deaths: i64,
    pub entry_diff: i64,
    pub plants: i64,
    pub defuses: i64,
    pub agent_pool: usize,
    /// Composite 0-1 score blending impact, survivability and winning.
    pub rating: f64,
    pub best_agents: Vec<AgentPlayerStat>,
    pub best_maps: Vec<PlayerMapStat>,
    pub timeline: Vec<PlayerTimelinePoint>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlayerMapStat {
    pub map: String,
    pub played: u32,
    pub win_rate: f64,
    pub rating: f64,
    pub kd: f64,
    pub avg_acs: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlayerTimelinePoint {
    pub match_id: i64,
    pub played_at: String,
    pub map: String,
    pub agent: String,
    pub result: &'static str,
    pub kd: f64,
    pub acs: i64,
}

pub fn player_stats(ds: &Dataset) -> Vec<PlayerStat> {
    let prior = ds.baseline_win_rate();
    let match_by_id: HashMap<i64, &MatchRow> = ds.matches.iter().map(|m| (m.id, m)).collect();

    // League-wide averages let a player's rating be read relative to the stack.
    let team_acs = ratio(
        ds.perfs.iter().map(|p| p.acs).sum::<i64>() as f64,
        ds.perfs.len() as f64,
    )
    .max(1.0);

    let mut totals: BTreeMap<i64, Tally> = BTreeMap::new();
    let mut by_agent: BTreeMap<(i64, String), Tally> = BTreeMap::new();
    let mut by_map: BTreeMap<(i64, String), Tally> = BTreeMap::new();
    let mut timelines: BTreeMap<i64, Vec<PlayerTimelinePoint>> = BTreeMap::new();

    for p in &ds.perfs {
        let Some(m) = match_by_id.get(&p.match_id) else {
            continue;
        };
        for t in [
            totals.entry(p.player_id).or_default(),
            by_agent.entry((p.player_id, p.agent.clone())).or_default(),
            by_map.entry((p.player_id, m.map.clone())).or_default(),
        ] {
            t.add_match(m);
            t.add_perf(p);
        }
        timelines
            .entry(p.player_id)
            .or_default()
            .push(PlayerTimelinePoint {
                match_id: m.id,
                played_at: m.played_at.clone(),
                map: m.map.clone(),
                agent: p.agent.clone(),
                result: result_of(m.rounds_won, m.rounds_lost),
                kd: round2(kd(p.kills as f64, p.deaths as f64)),
                acs: p.acs,
            });
    }

    let mut out: Vec<PlayerStat> = ds
        .players
        .iter()
        .map(|player| {
            let t = totals.get(&player.id).cloned().unwrap_or_default();

            let mut best_agents: Vec<AgentPlayerStat> = by_agent
                .iter()
                .filter(|((pid, _), _)| *pid == player.id)
                .map(|((_, agent), at)| AgentPlayerStat {
                    agent: agent.clone(),
                    player_id: player.id,
                    player: player.name.clone(),
                    picks: at.played,
                    win_rate: round3(at.win_rate()),
                    rating: round3(smoothed(at.wins as f64, (at.wins + at.losses) as f64, prior)),
                    kd: round2(at.kd()),
                    avg_acs: round2(at.avg_acs()),
                })
                .collect();
            // Rank by lower bound so a big sample beats a lucky two-game cameo.
            best_agents.sort_by(|a, b| {
                wilson_lower(a.win_rate * a.picks as f64, a.picks as f64)
                    .total_cmp(&wilson_lower(b.win_rate * b.picks as f64, b.picks as f64))
                    .reverse()
                    .then(b.picks.cmp(&a.picks))
            });

            let mut agent_names: Vec<String> = by_agent
                .iter()
                .filter(|((pid, _), _)| *pid == player.id)
                .map(|((_, a), _)| a.clone())
                .collect();
            agent_names.sort();
            agent_names.dedup();

            let mut best_maps: Vec<PlayerMapStat> = by_map
                .iter()
                .filter(|((pid, _), _)| *pid == player.id)
                .map(|((_, map), mt)| PlayerMapStat {
                    map: map.clone(),
                    played: mt.played,
                    win_rate: round3(mt.win_rate()),
                    rating: round3(smoothed(mt.wins as f64, (mt.wins + mt.losses) as f64, prior)),
                    kd: round2(mt.kd()),
                    avg_acs: round2(mt.avg_acs()),
                })
                .collect();
            best_maps.sort_by(|a, b| {
                wilson_lower(a.win_rate * a.played as f64, a.played as f64)
                    .total_cmp(&wilson_lower(b.win_rate * b.played as f64, b.played as f64))
                    .reverse()
                    .then(b.played.cmp(&a.played))
            });

            // Rating: impact (ACS vs. the stack), trading (K/D) and results,
            // squashed into 0-1 so it can drive a meter in the UI.
            let acs_component = (t.avg_acs() / team_acs / 2.0).clamp(0.0, 1.0);
            let kd_component = (t.kd() / 2.0).clamp(0.0, 1.0);
            let win_component = smoothed(t.wins as f64, (t.wins + t.losses) as f64, prior);
            let rating = 0.4 * acs_component + 0.35 * kd_component + 0.25 * win_component;

            let mut timeline = timelines.get(&player.id).cloned().unwrap_or_default();
            timeline.sort_by(|a, b| a.played_at.cmp(&b.played_at).then(a.match_id.cmp(&b.match_id)));

            PlayerStat {
                player_id: player.id,
                name: player.name.clone(),
                riot_id: player.riot_id.clone(),
                role: player.role.clone(),
                rank: player.rank.clone(),
                active: player.active,
                matches: t.played,
                wins: t.wins,
                win_rate: round3(t.win_rate()),
                kills: t.kills,
                deaths: t.deaths,
                assists: t.assists,
                kd: round2(t.kd()),
                kda: round2(ratio(
                    (t.kills + t.assists) as f64,
                    t.deaths.max(1) as f64,
                )),
                avg_kills: round2(ratio(t.kills as f64, t.played as f64)),
                avg_deaths: round2(ratio(t.deaths as f64, t.played as f64)),
                avg_acs: round2(t.avg_acs()),
                first_bloods: t.first_bloods,
                first_deaths: t.first_deaths,
                entry_diff: t.first_bloods - t.first_deaths,
                plants: t.plants,
                defuses: t.defuses,
                agent_pool: agent_names.len(),
                rating: round3(rating),
                best_agents: best_agents.into_iter().take(8).collect(),
                // Not truncated: the roster view and the player x map heatmap
                // both need every map a player has actually been on.
                best_maps,
                timeline,
            }
        })
        .collect();

    out.sort_by(|a, b| b.rating.total_cmp(&a.rating).then(a.name.cmp(&b.name)));
    out
}

// ---------------------------------------------------------------------------
// Compositions
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct CompStat {
    pub agents: Vec<String>,
    pub roles: Vec<&'static str>,
    pub played: u32,
    pub wins: u32,
    pub win_rate: f64,
    pub rating: f64,
    pub maps: Vec<String>,
    pub last_played: Option<String>,
}

pub fn comp_stats(ds: &Dataset) -> Vec<CompStat> {
    let prior = ds.baseline_win_rate();
    let by_match = ds.perfs_by_match();
    let mut comps: BTreeMap<Vec<String>, (Tally, BTreeMap<String, u32>)> = BTreeMap::new();

    for m in &ds.matches {
        let Some(perfs) = by_match.get(&m.id) else {
            continue;
        };
        if perfs.len() < 2 {
            continue;
        }
        let mut agents: Vec<String> = perfs.iter().map(|p| p.agent.clone()).collect();
        agents.sort();
        let entry = comps.entry(agents).or_default();
        entry.0.add_match(m);
        *entry.1.entry(m.map.clone()).or_default() += 1;
    }

    let mut out: Vec<CompStat> = comps
        .into_iter()
        .map(|(agents, (t, maps))| {
            let mut maps: Vec<(String, u32)> = maps.into_iter().collect();
            maps.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
            CompStat {
                roles: agents.iter().map(|a| catalog::role_of(a)).collect(),
                played: t.played,
                wins: t.wins,
                win_rate: round3(t.win_rate()),
                rating: round3(smoothed(t.wins as f64, (t.wins + t.losses) as f64, prior)),
                maps: maps.into_iter().map(|(m, _)| m).collect(),
                last_played: t.last_played.clone(),
                agents,
            }
        })
        .collect();

    out.sort_by(|a, b| {
        b.played
            .cmp(&a.played)
            .then(b.rating.total_cmp(&a.rating))
    });
    out
}
