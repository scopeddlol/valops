//! HenrikDev unofficial VALORANT API.
//!
//! **The response mapping in this file is written from the documented endpoint
//! and auth scheme but has not been run against the live API**, because the
//! host is unreachable from the environment this was built in. It is therefore
//! deliberately *tolerant*: every field is looked up through a list of
//! candidate paths covering the shapes v2 and v4 are documented to use, and
//! [`probe`] reports exactly which paths resolved and which did not.
//!
//! If a sync comes back empty or wrong, run the probe: it prints the keys the
//! API actually returned next to the ones this code looked for, which turns a
//! guess into a one-line fix.

use serde::Serialize;
use serde_json::Value;

use crate::error::{AppError, AppResult};
use crate::import::{IncomingMatch, IncomingPerformance, PlayerRef};

const BASE: &str = "https://api.henrikdev.xyz";
pub const REGIONS: [&str; 6] = ["eu", "na", "latam", "br", "ap", "kr"];

#[derive(Debug, Clone)]
pub struct HenrikConfig {
    pub api_key: String,
    pub region: String,
    pub platform: String,
}

impl HenrikConfig {
    /// Read the key from the environment; the region and platform come from
    /// the request so one deployment can serve players in different regions.
    pub fn from_env(region: &str, platform: &str) -> AppResult<Self> {
        let api_key = std::env::var("VALOPS_HENRIK_KEY").unwrap_or_default();
        if api_key.trim().is_empty() {
            return Err(AppError::BadRequest(
                "No API key configured. Set VALOPS_HENRIK_KEY to a HenrikDev key and restart."
                    .into(),
            ));
        }
        let region = region.trim().to_lowercase();
        if !REGIONS.contains(&region.as_str()) {
            return Err(AppError::BadRequest(format!(
                "Unknown region '{region}'. Expected one of: {}",
                REGIONS.join(", ")
            )));
        }
        Ok(Self {
            api_key,
            region,
            platform: match platform.trim().to_lowercase().as_str() {
                "" => "pc".to_string(),
                other => other.to_string(),
            },
        })
    }
}

/// What the extractor found, so a mismatch is diagnosable without a debugger.
#[derive(Debug, Default, Serialize)]
pub struct ProbeReport {
    pub matches_seen: usize,
    /// field -> the candidate path that resolved it.
    pub resolved: Vec<(String, String)>,
    /// Fields no candidate path matched.
    pub missing: Vec<String>,
    /// The actual keys present, so unmatched fields can be located by eye.
    pub observed_keys: Vec<(String, Vec<String>)>,
    pub notes: Vec<String>,
}

impl ProbeReport {
    fn record(&mut self, field: &str, path: Option<&str>) {
        match path {
            Some(p) => {
                if !self.resolved.iter().any(|(f, _)| f == field) {
                    self.resolved.push((field.to_string(), p.to_string()));
                }
            }
            None => {
                if !self.missing.iter().any(|f| f == field) {
                    self.missing.push(field.to_string());
                }
            }
        }
    }

    fn observe(&mut self, label: &str, value: &Value) {
        if self.observed_keys.iter().any(|(l, _)| l == label) {
            return;
        }
        if let Some(obj) = value.as_object() {
            self.observed_keys
                .push((label.to_string(), obj.keys().cloned().collect()));
        }
    }
}

/// Resolve a dotted path like `metadata.map.name` against a JSON value.
fn at<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    let mut cursor = value;
    for segment in path.split('.') {
        cursor = match segment.parse::<usize>() {
            Ok(i) => cursor.get(i)?,
            Err(_) => cursor.get(segment)?,
        };
        if cursor.is_null() {
            return None;
        }
    }
    Some(cursor)
}

/// First candidate path that resolves, with the path that won.
fn pick<'a>(value: &'a Value, paths: &[&'static str]) -> Option<(&'a Value, &'static str)> {
    paths.iter().find_map(|p| at(value, p).map(|v| (v, *p)))
}

/// Like [`pick`], but skips candidates that resolve to the wrong shape.
///
/// This matters: in the v2 response `players` is an *object* wrapping
/// `all_players`, while in v4 `players` is the array itself. Picking on
/// existence alone would stop at the object and find no scoreboard.
fn pick_array<'a>(value: &'a Value, paths: &[&'static str]) -> Option<(&'a Vec<Value>, &'static str)> {
    paths
        .iter()
        .find_map(|p| at(value, p).and_then(|v| v.as_array()).map(|v| (v, *p)))
}

fn pick_str(value: &Value, paths: &[&'static str]) -> Option<(String, &'static str)> {
    pick(value, paths).and_then(|(v, p)| match v {
        Value::String(s) if !s.is_empty() => Some((s.clone(), p)),
        Value::Number(n) => Some((n.to_string(), p)),
        _ => None,
    })
}

fn pick_i64(value: &Value, paths: &[&'static str]) -> Option<(i64, &'static str)> {
    pick(value, paths).and_then(|(v, p)| match v {
        Value::Number(n) => n.as_i64().or_else(|| n.as_f64().map(|f| f.round() as i64)).map(|i| (i, p)),
        Value::String(s) => s.parse::<f64>().ok().map(|f| (f.round() as i64, p)),
        Value::Bool(b) => Some((i64::from(*b), p)),
        _ => None,
    })
}

// --- candidate paths, covering both documented response generations --------

const P_MATCH_ID: &[&str] = &["metadata.match_id", "metadata.matchid", "match_id", "id"];
const P_MAP: &[&str] = &["metadata.map.name", "metadata.map", "map.name", "map"];
const P_MODE: &[&str] = &[
    "metadata.queue.name",
    "metadata.queue.mode_type",
    "metadata.queue",
    "metadata.mode",
    "metadata.game_mode",
];
const P_STARTED: &[&str] = &[
    "metadata.started_at",
    "metadata.game_start_patched",
    "metadata.game_start",
];
const P_PLAYERS: &[&str] = &["players", "players.all_players", "all_players"];
const P_TEAMS: &[&str] = &["teams"];

const P_PUUID: &[&str] = &["puuid"];
const P_NAME: &[&str] = &["name"];
const P_TAG: &[&str] = &["tag"];
const P_TEAM: &[&str] = &["team_id", "team"];
const P_AGENT: &[&str] = &["agent.name", "agent", "character"];
const P_KILLS: &[&str] = &["stats.kills", "kills"];
const P_DEATHS: &[&str] = &["stats.deaths", "deaths"];
const P_ASSISTS: &[&str] = &["stats.assists", "assists"];
const P_SCORE: &[&str] = &["stats.score", "score"];

/// Convert one match object. `roster` is the set of identifiers we care about
/// (lowercased puuids and "name#tag"s), used to decide which side was "us".
fn map_match(raw: &Value, roster: &[String], report: &mut ProbeReport) -> Option<IncomingMatch> {
    report.observe("match", raw);
    if let Some(md) = at(raw, "metadata") {
        report.observe("metadata", md);
    }

    let match_id = pick_str(raw, P_MATCH_ID);
    report.record("match_id", match_id.as_ref().map(|(_, p)| *p));
    let map = pick_str(raw, P_MAP);
    report.record("map", map.as_ref().map(|(_, p)| *p));
    let mode = pick_str(raw, P_MODE);
    report.record("mode", mode.as_ref().map(|(_, p)| *p));
    let started = pick_str(raw, P_STARTED).or_else(|| {
        // An epoch timestamp comes through as a number.
        pick_i64(raw, P_STARTED).map(|(secs, p)| {
            let dt = chrono::DateTime::from_timestamp(secs, 0).unwrap_or_default();
            (dt.format("%Y-%m-%dT%H:%M:%S").to_string(), p)
        })
    });
    report.record("started_at", started.as_ref().map(|(_, p)| *p));

    let players_node = pick_array(raw, P_PLAYERS);
    report.record("players", players_node.as_ref().map(|(_, p)| *p));
    let players = players_node?.0;
    if let Some(first) = players.first() {
        report.observe("player", first);
        if let Some(stats) = at(first, "stats") {
            report.observe("player.stats", stats);
        }
    }

    // --- which side were we on? ----------------------------------------
    let mut our_team: Option<String> = None;
    for p in players {
        let puuid = pick_str(p, P_PUUID).map(|(v, _)| v.to_lowercase());
        let handle = match (pick_str(p, P_NAME), pick_str(p, P_TAG)) {
            (Some((n, _)), Some((t, _))) => Some(format!("{n}#{t}").to_lowercase()),
            (Some((n, _)), None) => Some(n.to_lowercase()),
            _ => None,
        };
        let is_ours = puuid.as_ref().is_some_and(|p| roster.contains(p))
            || handle.as_ref().is_some_and(|h| roster.contains(h));
        if is_ours {
            if let Some((team, _)) = pick_str(p, P_TEAM) {
                our_team = Some(team);
                break;
            }
        }
    }
    let our_team = our_team?;

    // --- the scoreline from our side ------------------------------------
    let teams_node = pick(raw, P_TEAMS);
    report.record("teams", teams_node.as_ref().map(|(_, p)| *p));
    let (rounds_won, rounds_lost) = teams_node
        .and_then(|(teams, _)| {
            if let Some(first) = teams.as_array().and_then(|a| a.first()) {
                report.observe("team", first);
            }
            team_rounds(teams, &our_team)
        })
        .unwrap_or((0, 0));
    report.record(
        "rounds",
        (rounds_won + rounds_lost > 0).then_some("teams[].rounds"),
    );

    let rounds_played = (rounds_won + rounds_lost).max(1);

    let performances = players
        .iter()
        .filter(|p| {
            pick_str(p, P_TEAM)
                .map(|(t, _)| t.eq_ignore_ascii_case(&our_team))
                .unwrap_or(false)
        })
        .filter_map(|p| {
            let puuid = pick_str(p, P_PUUID).map(|(v, _)| v);
            let name = pick_str(p, P_NAME).map(|(v, _)| v);
            let tag = pick_str(p, P_TAG).map(|(v, _)| v);
            let riot_id = match (&name, &tag) {
                (Some(n), Some(t)) => Some(format!("{n}#{t}")),
                _ => None,
            };
            let who = puuid
                .clone()
                .map(PlayerRef::Puuid)
                .or_else(|| riot_id.clone().map(PlayerRef::RiotId))
                .or_else(|| name.clone().map(PlayerRef::Name))?;

            let agent = pick_str(p, P_AGENT);
            report.record("agent", agent.as_ref().map(|(_, pa)| *pa));
            let kills = pick_i64(p, P_KILLS);
            report.record("kills", kills.as_ref().map(|(_, pa)| *pa));
            let deaths = pick_i64(p, P_DEATHS);
            report.record("deaths", deaths.as_ref().map(|(_, pa)| *pa));
            let assists = pick_i64(p, P_ASSISTS);
            report.record("assists", assists.as_ref().map(|(_, pa)| *pa));
            let score = pick_i64(p, P_SCORE);
            report.record("score", score.as_ref().map(|(_, pa)| *pa));

            Some(IncomingPerformance {
                player: who,
                riot_id,
                puuid,
                agent: agent.map(|(v, _)| v).unwrap_or_else(|| "Unknown".into()),
                kills: kills.map(|(v, _)| v).unwrap_or(0),
                deaths: deaths.map(|(v, _)| v).unwrap_or(0),
                assists: assists.map(|(v, _)| v).unwrap_or(0),
                acs: score.map(|(v, _)| to_acs(v, rounds_played)).unwrap_or(0),
                // Opening duels need round-by-round parsing, which this source
                // does not expose at the match level. Left at zero rather than
                // guessed — the app shows entry stats as 0 for imported games.
                first_bloods: 0,
                first_deaths: 0,
                plants: 0,
                defuses: 0,
            })
        })
        .collect::<Vec<_>>();

    if performances.is_empty() {
        return None;
    }

    Some(IncomingMatch {
        external_id: match_id.map(|(v, _)| v),
        source: "henrikdev".into(),
        played_at: started
            .map(|(v, _)| normalise_timestamp(&v))
            .unwrap_or_else(|| chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string()),
        map: map.map(|(v, _)| v).unwrap_or_else(|| "Unknown".into()),
        mode: mode.map(|(v, _)| v).unwrap_or_else(|| "Competitive".into()),
        rounds_won,
        rounds_lost,
        notes: None,
        performances,
    })
}

/// Rounds for our team and theirs, across both documented `teams` shapes: a v4
/// array of `{team_id, rounds:{won,lost}}` and a v2 object of `{red:{...},
/// blue:{...}}`.
fn team_rounds(teams: &Value, our_team: &str) -> Option<(i64, i64)> {
    if let Some(list) = teams.as_array() {
        let mut ours = None;
        let mut theirs = None;
        for t in list {
            let id = pick_str(t, &["team_id", "team"]).map(|(v, _)| v)?;
            let won = pick_i64(t, &["rounds.won", "rounds_won", "rounds.win"]).map(|(v, _)| v)?;
            if id.eq_ignore_ascii_case(our_team) {
                ours = Some(won);
            } else {
                theirs = Some(won);
            }
        }
        return Some((ours?, theirs?));
    }
    if teams.is_object() {
        // Addressed by side name rather than by candidate order, which has no
        // way of knowing which of red/blue is ours.
        let side = our_team.to_lowercase();
        let other = if side == "red" { "blue" } else { "red" };
        let rounds_of = |team: &str| {
            at(teams, &format!("{team}.rounds_won"))
                .or_else(|| at(teams, &format!("{team}.rounds.won")))
                .and_then(|v| v.as_i64())
        };
        return Some((rounds_of(&side)?, rounds_of(other)?));
    }
    None
}

/// The API reports total combat score; the app stores average combat score.
/// If the number already looks like an ACS, it is left alone.
fn to_acs(score: i64, rounds_played: i64) -> i64 {
    let per_round = score / rounds_played.max(1);
    if (30..=600).contains(&per_round) {
        per_round
    } else if (30..=600).contains(&score) {
        score
    } else {
        per_round
    }
}

/// Accept the several timestamp spellings and store a plain local-ish ISO
/// string, which is what the rest of the app compares and sorts on.
fn normalise_timestamp(raw: &str) -> String {
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(raw) {
        return dt.format("%Y-%m-%dT%H:%M:%S").to_string();
    }
    for fmt in ["%A, %B %d, %Y %l:%M %p", "%Y-%m-%d %H:%M:%S", "%Y-%m-%dT%H:%M:%S"] {
        if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(raw.trim(), fmt) {
            return dt.format("%Y-%m-%dT%H:%M:%S").to_string();
        }
    }
    raw.to_string()
}

async fn get(cfg: &HenrikConfig, path: &str) -> AppResult<Value> {
    let url = format!("{BASE}{path}");
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .user_agent("valops/0.1 (+self-hosted five-stack tracker)")
        .build()
        .map_err(|e| AppError::BadRequest(format!("Could not start the HTTP client: {e}")))?;

    let res = client
        .get(&url)
        .header("Authorization", &cfg.api_key)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| AppError::BadRequest(format!("Could not reach the API: {e}")))?;

    let status = res.status();
    let body = res.text().await.unwrap_or_default();

    if !status.is_success() {
        // Pass the API's own message through; its 4xx bodies explain themselves
        // far better than a generic error would.
        let detail = serde_json::from_str::<Value>(&body)
            .ok()
            .and_then(|v| {
                at(&v, "errors.0.message")
                    .or_else(|| at(&v, "error"))
                    .or_else(|| at(&v, "message"))
                    .and_then(|m| m.as_str().map(str::to_string))
            })
            .unwrap_or_else(|| body.chars().take(200).collect());
        return Err(AppError::BadRequest(match status.as_u16() {
            401 | 403 => format!("The API rejected the key ({status}). {detail}"),
            404 => format!("No such player or no matches found ({status}). {detail}"),
            429 => "Rate limited by the API — wait a minute and try again.".to_string(),
            _ => format!("The API returned {status}. {detail}"),
        }));
    }

    serde_json::from_str(&body)
        .map_err(|e| AppError::BadRequest(format!("The API returned something that is not JSON: {e}")))
}

fn matches_array(payload: &Value) -> Option<&Vec<Value>> {
    payload
        .get("data")
        .and_then(|d| d.as_array())
        .or_else(|| payload.as_array())
}

/// Pull a player's recent matches and normalise them.
///
/// `roster` holds the identifiers (lowercased puuids and `name#tag`s) that
/// count as "us"; only those players' rows are kept, and their side of the
/// scoreline becomes rounds won/lost.
pub async fn fetch(
    cfg: &HenrikConfig,
    name: &str,
    tag: &str,
    roster: &[String],
    mode_filter: Option<&str>,
    size: usize,
) -> AppResult<(Vec<IncomingMatch>, ProbeReport)> {
    let path = format!(
        "/valorant/v4/matches/{}/{}/{}/{}?size={}",
        cfg.region,
        cfg.platform,
        urlencode(name),
        urlencode(tag),
        size.clamp(1, 20),
    );
    let payload = get(cfg, &path).await?;

    let mut report = ProbeReport::default();
    report.observe("response", &payload);

    let Some(list) = matches_array(&payload) else {
        report.notes.push(
            "Could not find a match array — expected `data` to be a list at the top level."
                .to_string(),
        );
        return Ok((Vec::new(), report));
    };
    report.matches_seen = list.len();

    let mut out = Vec::new();
    for raw in list {
        let Some(m) = map_match(raw, roster, &mut report) else {
            continue;
        };
        if let Some(want) = mode_filter {
            if !want.is_empty() && !m.mode.eq_ignore_ascii_case(want) {
                continue;
            }
        }
        out.push(m);
    }

    if out.is_empty() && report.matches_seen > 0 {
        report.notes.push(
            "Matches came back but none mapped. Most often this means no roster player was \
             recognised in them — check that the Riot IDs on your roster match, or add puuids."
                .to_string(),
        );
    }
    if !report.missing.is_empty() {
        report.notes.push(format!(
            "Unmapped fields: {}. The observed_keys below show what the API actually returned.",
            report.missing.join(", ")
        ));
    }
    Ok((out, report))
}

/// Percent-encode a path segment. Riot names allow spaces and a few other
/// characters that must not go into a URL raw.
fn urlencode(input: &str) -> String {
    input
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            other => format!("%{other:02X}"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A v4-shaped payload, as documented.
    fn v4_match() -> Value {
        serde_json::json!({
            "metadata": {
                "match_id": "abc-123",
                "map": { "name": "Ascent" },
                "queue": { "name": "Competitive" },
                "started_at": "2026-09-14T21:30:00Z"
            },
            "players": [
                { "puuid": "p-vex", "name": "Vex", "tag": "EUW", "team_id": "Red",
                  "agent": { "name": "Jett" },
                  "stats": { "kills": 22, "deaths": 14, "assists": 5, "score": 5400 } },
                { "puuid": "p-nyx", "name": "Nyx", "tag": "0001", "team_id": "Red",
                  "agent": { "name": "Omen" },
                  "stats": { "kills": 15, "deaths": 16, "assists": 9, "score": 4200 } },
                { "puuid": "p-rando", "name": "Enemy", "tag": "XXX", "team_id": "Blue",
                  "agent": { "name": "Raze" },
                  "stats": { "kills": 19, "deaths": 18, "assists": 3, "score": 4800 } }
            ],
            "teams": [
                { "team_id": "Red", "rounds": { "won": 13, "lost": 9 } },
                { "team_id": "Blue", "rounds": { "won": 9, "lost": 13 } }
            ]
        })
    }

    /// The older v2 shape, to prove the tolerant paths earn their keep.
    fn v2_match() -> Value {
        serde_json::json!({
            "metadata": {
                "matchid": "old-456",
                "map": "Bind",
                "mode": "Competitive",
                "game_start": 1_757_000_000i64
            },
            "players": { "all_players": [
                { "puuid": "p-vex", "name": "Vex", "tag": "EUW", "team": "Blue",
                  "character": "Raze", "stats": { "kills": 20, "deaths": 12, "assists": 4, "score": 4600 } }
            ]},
            "teams": { "blue": { "rounds_won": 13 }, "red": { "rounds_won": 7 } }
        })
    }

    fn roster() -> Vec<String> {
        vec!["p-vex".into(), "p-nyx".into(), "vex#euw".into()]
    }

    #[test]
    fn maps_a_v4_match_from_our_side() {
        let mut report = ProbeReport::default();
        let m = map_match(&v4_match(), &roster(), &mut report).expect("should map");
        assert_eq!(m.external_id.as_deref(), Some("abc-123"));
        assert_eq!(m.map, "Ascent");
        assert_eq!(m.mode, "Competitive");
        assert_eq!(m.played_at, "2026-09-14T21:30:00");
        assert_eq!((m.rounds_won, m.rounds_lost), (13, 9));
        // Only our side's players are kept.
        assert_eq!(m.performances.len(), 2);
        assert_eq!(m.performances[0].agent, "Jett");
        // Total combat score becomes ACS: 5400 / 22 rounds.
        assert_eq!(m.performances[0].acs, 245);
        assert_eq!(m.source, "henrikdev");
    }

    #[test]
    fn maps_the_older_v2_shape_too() {
        let mut report = ProbeReport::default();
        let m = map_match(&v2_match(), &roster(), &mut report).expect("should map");
        assert_eq!(m.external_id.as_deref(), Some("old-456"));
        assert_eq!(m.map, "Bind");
        assert_eq!((m.rounds_won, m.rounds_lost), (13, 7));
        assert_eq!(m.performances[0].agent, "Raze");
    }

    #[test]
    fn skips_a_match_no_roster_player_was_in() {
        let mut report = ProbeReport::default();
        assert!(map_match(&v4_match(), &["someone-else".into()], &mut report).is_none());
    }

    #[test]
    fn losing_side_gets_the_scoreline_the_right_way_round() {
        let mut raw = v4_match();
        raw["players"][0]["team_id"] = serde_json::json!("Blue");
        raw["players"][1]["team_id"] = serde_json::json!("Blue");
        let mut report = ProbeReport::default();
        let m = map_match(&raw, &roster(), &mut report).unwrap();
        assert_eq!((m.rounds_won, m.rounds_lost), (9, 13));
    }

    #[test]
    fn probe_reports_what_it_could_not_map() {
        let mut raw = v4_match();
        raw["metadata"]["map"] = serde_json::json!(null);
        let mut report = ProbeReport::default();
        map_match(&raw, &roster(), &mut report).unwrap();
        assert!(report.missing.contains(&"map".to_string()));
        assert!(report.resolved.iter().any(|(f, p)| f == "kills" && *p == "stats.kills"));
    }

    #[test]
    fn already_averaged_scores_are_not_divided_again() {
        assert_eq!(to_acs(5400, 22), 245); // total combat score
        assert_eq!(to_acs(245, 1), 245); // already an average
    }

    #[test]
    fn names_with_spaces_are_url_safe() {
        assert_eq!(urlencode("Some Name"), "Some%20Name");
        assert_eq!(urlencode("Vex"), "Vex");
    }
}
