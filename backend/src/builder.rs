//! Team builder: given a map and a roster, pick the five agents the stack has
//! actually been winning with.
//!
//! Every candidate agent is scored per player from that player's own history,
//! then an exact search over the top candidates picks the best legal five-agent
//! comp (no duplicate agents, sane role spread). Because each component of the
//! score is kept separately, every recommendation can explain itself.

use std::collections::{BTreeMap, HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::analytics::{comp_stats, round2, round3, smoothed, CompStat, Dataset};
use crate::catalog;
use crate::models::MatchRow;

/// How many agents per player enter the combinatorial search.
const CANDIDATES_PER_PLAYER: usize = 8;
/// Picks needed before an agent counts as fully "comfortable".
const COMFORT_TARGET: f64 = 8.0;

#[derive(Debug, Deserialize)]
pub struct BuilderQuery {
    pub map: String,
    /// Comma-separated player ids. Defaults to the active roster.
    pub players: Option<String>,
    /// Comma-separated `player_id:Agent` pairs to pin in place.
    pub locks: Option<String>,
    /// Comma-separated agents to keep out of the comp.
    pub exclude: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AgentOption {
    pub agent: String,
    pub role: &'static str,
    pub score: f64,
    pub win_rate_on_map: Option<f64>,
    pub win_rate_overall: Option<f64>,
    pub picks_on_map: u32,
    pub picks_overall: u32,
    pub kd: Option<f64>,
    pub avg_acs: Option<f64>,
    /// 0-1: how much history backs this suggestion.
    pub confidence: f64,
}

#[derive(Debug, Serialize)]
pub struct BuilderSlot {
    pub player_id: i64,
    pub player: String,
    pub preferred_role: String,
    pub locked: bool,
    #[serde(flatten)]
    pub pick: AgentOption,
    pub reasons: Vec<String>,
    pub alternatives: Vec<AgentOption>,
}

#[derive(Debug, Serialize)]
pub struct BuilderResult {
    pub map: String,
    pub slots: Vec<BuilderSlot>,
    pub role_spread: BTreeMap<String, u32>,
    pub comp_score: f64,
    pub confidence: f64,
    pub notes: Vec<String>,
    /// The team's record with this exact five-agent comp, if they have run it.
    pub history: Option<CompStat>,
    pub map_record: Option<String>,
}

#[derive(Debug, Default, Clone, Copy)]
struct Acc {
    played: u32,
    wins: u32,
    kills: i64,
    deaths: i64,
    acs: i64,
}

impl Acc {
    fn add(&mut self, m: &MatchRow, kills: i64, deaths: i64, acs: i64) {
        self.played += 1;
        if m.rounds_won > m.rounds_lost {
            self.wins += 1;
        }
        self.kills += kills;
        self.deaths += deaths;
        self.acs += acs;
    }
    fn win_rate(&self) -> f64 {
        if self.played == 0 {
            0.0
        } else {
            self.wins as f64 / self.played as f64
        }
    }
    fn kd(&self) -> f64 {
        if self.deaths == 0 {
            self.kills as f64
        } else {
            self.kills as f64 / self.deaths as f64
        }
    }
    fn avg_acs(&self) -> f64 {
        if self.played == 0 {
            0.0
        } else {
            self.acs as f64 / self.played as f64
        }
    }
}

/// Map a ratio-to-personal-baseline onto 0-1, where "same as usual" sits at 0.5.
fn relative(value: f64, baseline: f64) -> f64 {
    if baseline <= 0.0 || value <= 0.0 {
        return 0.5;
    }
    (0.5 + (value / baseline - 1.0) * 1.5).clamp(0.0, 1.0)
}

pub fn build(ds: &Dataset, query: &BuilderQuery) -> BuilderResult {
    let map = query.map.trim().to_string();
    let prior = ds.baseline_win_rate();

    let locks: HashMap<i64, String> = query
        .locks
        .as_deref()
        .unwrap_or("")
        .split(',')
        .filter_map(|pair| {
            let (id, agent) = pair.split_once(':')?;
            let id = id.trim().parse::<i64>().ok()?;
            let agent = agent.trim();
            if agent.is_empty() {
                return None;
            }
            Some((id, canonical_agent(agent)))
        })
        .collect();

    let excluded: HashSet<String> = query
        .exclude
        .as_deref()
        .unwrap_or("")
        .split(',')
        .map(|a| canonical_agent(a.trim()))
        .filter(|a| !a.is_empty())
        .collect();

    let roster: Vec<&crate::models::Player> = match &query.players {
        Some(list) if !list.trim().is_empty() => {
            let ids: Vec<i64> = list
                .split(',')
                .filter_map(|s| s.trim().parse::<i64>().ok())
                .collect();
            ids.iter()
                .filter_map(|id| ds.players.iter().find(|p| p.id == *id))
                .collect()
        }
        _ => ds.players.iter().filter(|p| p.active).collect(),
    };

    // --- fold history into the lookups the scorer needs -------------------
    let match_by_id: HashMap<i64, &MatchRow> = ds.matches.iter().map(|m| (m.id, m)).collect();
    let mut pam: HashMap<(i64, String), Acc> = HashMap::new(); // player+agent on this map
    let mut pa: HashMap<(i64, String), Acc> = HashMap::new(); // player+agent anywhere
    let mut ptotal: HashMap<i64, Acc> = HashMap::new(); // player overall
    let mut team_agent_map: HashMap<String, Acc> = HashMap::new(); // agent on this map, any player

    for p in &ds.perfs {
        let Some(m) = match_by_id.get(&p.match_id) else {
            continue;
        };
        let agent = canonical_agent(&p.agent);
        pa.entry((p.player_id, agent.clone()))
            .or_default()
            .add(m, p.kills, p.deaths, p.acs);
        ptotal
            .entry(p.player_id)
            .or_default()
            .add(m, p.kills, p.deaths, p.acs);
        if m.map.eq_ignore_ascii_case(&map) {
            pam.entry((p.player_id, agent.clone()))
                .or_default()
                .add(m, p.kills, p.deaths, p.acs);
            team_agent_map
                .entry(agent)
                .or_default()
                .add(m, p.kills, p.deaths, p.acs);
        }
    }

    // --- score every (player, agent) pair ---------------------------------
    let mut per_player: Vec<(i64, Vec<AgentOption>)> = Vec::new();
    for player in &roster {
        let totals = ptotal.get(&player.id).copied().unwrap_or_default();
        let mut options: Vec<AgentOption> = catalog::AGENTS
            .iter()
            .filter(|a| !excluded.contains(a.name))
            .map(|a| {
                let key = (player.id, a.name.to_string());
                let on_map = pam.get(&key).copied().unwrap_or_default();
                let overall = pa.get(&key).copied().unwrap_or_default();
                let meta = team_agent_map.get(a.name).copied().unwrap_or_default();

                // Winning: mostly this player on this agent on this map, backed
                // by their record on the agent everywhere else.
                let wr_map = smoothed(on_map.wins as f64, on_map.played as f64, prior);
                let wr_any = smoothed(overall.wins as f64, overall.played as f64, prior);
                let winning = 0.6 * wr_map + 0.4 * wr_any;

                // Impact: how they perform on this agent versus their own norm.
                let impact = if overall.played == 0 {
                    0.5
                } else {
                    0.5 * relative(overall.avg_acs(), totals.avg_acs())
                        + 0.5 * relative(overall.kd(), totals.kd())
                };

                // Comfort: reps on the agent, saturating at COMFORT_TARGET.
                let comfort = ((overall.played as f64 + 0.5 * on_map.played as f64)
                    / COMFORT_TARGET)
                    .clamp(0.0, 1.0);

                // Role fit keeps cold-start suggestions sensible.
                let role_fit = if player.role.eq_ignore_ascii_case(a.role) {
                    1.0
                } else if player.role.eq_ignore_ascii_case("Flex") {
                    0.7
                } else {
                    0.3
                };

                // Map meta from the team's own games, not an outside tier list.
                let map_meta = smoothed(meta.wins as f64, meta.played as f64, prior);

                let score = 0.30 * winning
                    + 0.22 * impact
                    + 0.18 * comfort
                    + 0.15 * role_fit
                    + 0.15 * map_meta;

                let confidence =
                    ((on_map.played as f64 + 0.5 * overall.played as f64) / 10.0).clamp(0.0, 1.0);

                AgentOption {
                    agent: a.name.to_string(),
                    role: a.role,
                    score: round3(score),
                    win_rate_on_map: (on_map.played > 0).then(|| round3(on_map.win_rate())),
                    win_rate_overall: (overall.played > 0).then(|| round3(overall.win_rate())),
                    picks_on_map: on_map.played,
                    picks_overall: overall.played,
                    kd: (overall.played > 0).then(|| round2(overall.kd())),
                    avg_acs: (overall.played > 0).then(|| round2(overall.avg_acs())),
                    confidence: round3(confidence),
                }
            })
            .collect();

        options.sort_by(|a, b| {
            b.score
                .total_cmp(&a.score)
                .then(b.picks_overall.cmp(&a.picks_overall))
        });

        if let Some(locked) = locks.get(&player.id) {
            // A locked agent is the only candidate for that slot.
            if let Some(idx) = options.iter().position(|o| &o.agent == locked) {
                let chosen = options.remove(idx);
                per_player.push((player.id, vec![chosen]));
                continue;
            }
        }
        options.truncate(CANDIDATES_PER_PLAYER);
        per_player.push((player.id, options));
    }

    // --- search the best legal combination --------------------------------
    let best = search(&per_player);

    // --- assemble the response -------------------------------------------
    let mut role_spread: BTreeMap<String, u32> = catalog::ROLES
        .iter()
        .map(|r| (r.to_string(), 0))
        .collect();
    let mut slots = Vec::new();
    let mut score_sum = 0.0;
    let mut confidence_sum = 0.0;

    for (slot_idx, (player_id, options)) in per_player.iter().enumerate() {
        let player = roster.iter().find(|p| p.id == *player_id).unwrap();
        let chosen_idx = best.get(slot_idx).copied().unwrap_or(0);
        let Some(pick) = options.get(chosen_idx) else {
            continue;
        };
        *role_spread.entry(pick.role.to_string()).or_default() += 1;
        score_sum += pick.score;
        confidence_sum += pick.confidence;

        let alternatives: Vec<AgentOption> = options
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != chosen_idx)
            .take(4)
            .map(|(_, o)| clone_option(o))
            .collect();

        slots.push(BuilderSlot {
            player_id: *player_id,
            player: player.name.clone(),
            preferred_role: player.role.clone(),
            locked: locks.contains_key(player_id),
            reasons: reasons_for(pick, &player.role, &map),
            pick: clone_option(pick),
            alternatives,
        });
    }

    let n = slots.len().max(1) as f64;
    let mut notes = Vec::new();
    for role in ["Controller", "Initiator", "Sentinel", "Duelist"] {
        if role_spread.get(role).copied().unwrap_or(0) == 0 && slots.len() >= 4 {
            notes.push(format!("No {role} in this comp — the data still likes it, but expect a harder time."));
        }
    }
    if confidence_sum / n < 0.35 {
        notes.push(
            "Thin history on this map. Log a few more matches and these picks will sharpen up."
                .to_string(),
        );
    }

    // Has the stack run this exact five before?
    let mut agents: Vec<String> = slots.iter().map(|s| s.pick.agent.clone()).collect();
    agents.sort();
    let history = comp_stats(ds).into_iter().find(|c| c.agents == agents);

    let map_played = ds.matches.iter().filter(|m| m.map.eq_ignore_ascii_case(&map));
    let (played, wins) = map_played.fold((0u32, 0u32), |(p, w), m| {
        (p + 1, w + u32::from(m.rounds_won > m.rounds_lost))
    });
    let map_record = (played > 0).then(|| format!("{wins}-{} on {map}", played - wins));

    BuilderResult {
        map,
        comp_score: round3((score_sum / n + comp_bonus(&role_spread)).clamp(0.0, 1.0)),
        confidence: round3(confidence_sum / n),
        slots,
        role_spread,
        notes,
        history,
        map_record,
    }
}

fn clone_option(o: &AgentOption) -> AgentOption {
    AgentOption {
        agent: o.agent.clone(),
        role: o.role,
        score: o.score,
        win_rate_on_map: o.win_rate_on_map,
        win_rate_overall: o.win_rate_overall,
        picks_on_map: o.picks_on_map,
        picks_overall: o.picks_overall,
        kd: o.kd,
        avg_acs: o.avg_acs,
        confidence: o.confidence,
    }
}

fn pct(v: f64) -> String {
    format!("{:.0}%", v * 100.0)
}

fn reasons_for(pick: &AgentOption, preferred_role: &str, map: &str) -> Vec<String> {
    let mut out = Vec::new();
    match (pick.picks_on_map, pick.win_rate_on_map) {
        (n, Some(wr)) if n >= 3 => out.push(format!(
            "{} on {map} as {} ({} game{})",
            pct(wr),
            pick.agent,
            n,
            if n == 1 { "" } else { "s" }
        )),
        (n, Some(wr)) if n > 0 => out.push(format!(
            "{}-{} on {map} as {} — small sample",
            (wr * n as f64).round() as u32,
            n - (wr * n as f64).round() as u32,
            pick.agent
        )),
        _ => out.push(format!("No games on {} at {map} yet", pick.agent)),
    }
    if let (Some(kd), Some(acs)) = (pick.kd, pick.avg_acs) {
        out.push(format!("{kd:.2} K/D · {acs:.0} ACS on {}", pick.agent));
    }
    if preferred_role.eq_ignore_ascii_case(pick.role) {
        out.push(format!("Matches their {} role", pick.role));
    } else if !preferred_role.eq_ignore_ascii_case("Flex") {
        out.push(format!(
            "Off-role pick ({} playing {})",
            preferred_role, pick.role
        ));
    }
    out
}

/// Bonus/penalty for the shape of a comp, in score units.
fn comp_bonus(spread: &BTreeMap<String, u32>) -> f64 {
    let get = |r: &str| spread.get(r).copied().unwrap_or(0);
    let total: u32 = spread.values().sum();
    if total < 4 {
        return 0.0; // Partial rosters are not held to full-comp rules.
    }
    let mut bonus = 0.0;
    bonus += if get("Controller") >= 1 { 0.05 } else { -0.12 };
    bonus += if get("Initiator") >= 1 { 0.04 } else { -0.08 };
    bonus += if get("Sentinel") >= 1 { 0.03 } else { -0.06 };
    bonus += if get("Duelist") >= 1 { 0.03 } else { -0.06 };
    for role in catalog::ROLES {
        let count = get(role);
        if count > 2 {
            bonus -= 0.05 * (count - 2) as f64;
        }
    }
    bonus
}

/// Exhaustive search over the per-player candidate lists for the highest scoring
/// comp with no duplicate agents. With five players and eight candidates each
/// this is at most 32,768 combinations — fast enough to do on every request.
fn search(per_player: &[(i64, Vec<AgentOption>)]) -> Vec<usize> {
    let mut best_choice = vec![0usize; per_player.len()];
    let mut best_score = f64::NEG_INFINITY;
    let mut current = vec![0usize; per_player.len()];
    let mut used: HashSet<&str> = HashSet::new();

    fn recurse<'a>(
        idx: usize,
        per_player: &'a [(i64, Vec<AgentOption>)],
        current: &mut Vec<usize>,
        used: &mut HashSet<&'a str>,
        running: f64,
        best_score: &mut f64,
        best_choice: &mut Vec<usize>,
    ) {
        if idx == per_player.len() {
            let mut spread: BTreeMap<String, u32> = BTreeMap::new();
            for (i, (_, options)) in per_player.iter().enumerate() {
                if let Some(o) = options.get(current[i]) {
                    *spread.entry(o.role.to_string()).or_default() += 1;
                }
            }
            let n = per_player.len().max(1) as f64;
            let total = running / n + comp_bonus(&spread);
            if total > *best_score {
                *best_score = total;
                best_choice.clone_from(current);
            }
            return;
        }
        let options = &per_player[idx].1;
        if options.is_empty() {
            recurse(idx + 1, per_player, current, used, running, best_score, best_choice);
            return;
        }
        for (i, option) in options.iter().enumerate() {
            if used.contains(option.agent.as_str()) {
                continue;
            }
            used.insert(option.agent.as_str());
            current[idx] = i;
            recurse(
                idx + 1,
                per_player,
                current,
                used,
                running + option.score,
                best_score,
                best_choice,
            );
            used.remove(option.agent.as_str());
        }
    }

    recurse(
        0,
        per_player,
        &mut current,
        &mut used,
        0.0,
        &mut best_score,
        &mut best_choice,
    );
    best_choice
}

/// Normalise user-typed agent names to the catalog spelling ("jett" -> "Jett").
fn canonical_agent(input: &str) -> String {
    catalog::AGENTS
        .iter()
        .find(|a| a.name.eq_ignore_ascii_case(input))
        .map(|a| a.name.to_string())
        .unwrap_or_else(|| input.to_string())
}
