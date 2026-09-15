//! The one path data takes into the database, whatever its origin.
//!
//! A pasted export, a spreadsheet and an API sync all normalise to
//! [`IncomingMatch`] and go through [`apply`]. That means dedup, player
//! resolution and the dry-run preview are written once and behave identically
//! for every source — adding a source is a matter of producing these structs.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::error::{AppError, AppResult};
use crate::models::{result_of, PlayerInput};

/// How a scoreboard row names its player. Sources know different things: an
/// export knows our own ids, a spreadsheet knows names, an API knows puuids.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlayerRef {
    Puuid(String),
    /// "Name#TAG"
    RiotId(String),
    Name(String),
}

impl PlayerRef {
    pub fn label(&self) -> String {
        match self {
            PlayerRef::Puuid(p) => format!("puuid {}", &p[..p.len().min(8)]),
            PlayerRef::RiotId(r) => r.clone(),
            PlayerRef::Name(n) => n.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct IncomingPerformance {
    pub player: PlayerRef,
    /// Used to create the player if they are new and creation is enabled.
    pub riot_id: Option<String>,
    pub puuid: Option<String>,
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

/// One unit of work for [`apply`]: the matches, plus whatever the source knows
/// about the people in them.
#[derive(Debug, Default)]
pub struct ImportBatch {
    /// Roster metadata (role, rank, Riot ID). Without this a restore would
    /// rebuild players from scoreboard rows alone and silently drop all of it.
    pub players: Vec<PlayerInput>,
    pub matches: Vec<IncomingMatch>,
}

impl ImportBatch {
    pub fn matches_only(matches: Vec<IncomingMatch>) -> Self {
        Self { players: Vec::new(), matches }
    }
}

#[derive(Debug, Clone)]
pub struct IncomingMatch {
    /// The source's own id. Present means the import is idempotent.
    pub external_id: Option<String>,
    pub source: String,
    pub played_at: String,
    pub map: String,
    pub mode: String,
    pub rounds_won: i64,
    pub rounds_lost: i64,
    pub notes: Option<String>,
    pub performances: Vec<IncomingPerformance>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ImportOptions {
    /// Report what would happen without writing anything.
    #[serde(default)]
    pub dry_run: bool,
    /// Add players the import mentions but the roster does not have.
    #[serde(default = "default_true")]
    pub create_missing_players: bool,
    /// Skip a match that already looks present (same time, map and score),
    /// which is what makes re-importing an export safe.
    #[serde(default = "default_true")]
    pub skip_existing: bool,
}

fn default_true() -> bool {
    true
}

impl Default for ImportOptions {
    fn default() -> Self {
        Self { dry_run: false, create_missing_players: true, skip_existing: true }
    }
}

#[derive(Debug, Default, Serialize)]
pub struct ImportReport {
    pub dry_run: bool,
    pub imported: usize,
    pub skipped_duplicates: usize,
    pub skipped_empty: usize,
    pub performances: usize,
    /// Players this import would add (or added) to the roster.
    pub created_players: Vec<String>,
    /// Players it could not resolve and did not create — their rows were dropped.
    pub unknown_players: Vec<String>,
    pub warnings: Vec<String>,
    /// A line per match, so the UI can show exactly what is about to land.
    pub preview: Vec<PreviewRow>,
}

#[derive(Debug, Serialize)]
pub struct PreviewRow {
    pub played_at: String,
    pub map: String,
    pub mode: String,
    pub score: String,
    pub result: &'static str,
    pub players: usize,
    pub status: &'static str,
}

/// An in-memory index of the roster, so resolving a scoreboard is not N queries.
struct Roster {
    by_id: HashMap<i64, String>,
    by_puuid: HashMap<String, i64>,
    by_riot_id: HashMap<String, i64>,
    by_name: HashMap<String, i64>,
}

impl Roster {
    async fn load(pool: &SqlitePool) -> AppResult<Self> {
        let rows: Vec<(i64, String, Option<String>, Option<String>)> =
            sqlx::query_as("SELECT id, name, riot_id, puuid FROM players")
                .fetch_all(pool)
                .await?;
        let mut r = Self {
            by_id: HashMap::new(),
            by_puuid: HashMap::new(),
            by_riot_id: HashMap::new(),
            by_name: HashMap::new(),
        };
        for (id, name, riot_id, puuid) in rows {
            r.by_name.insert(name.to_lowercase(), id);
            r.by_id.insert(id, name);
            if let Some(p) = puuid {
                r.by_puuid.insert(p.to_lowercase(), id);
            }
            if let Some(rid) = riot_id {
                r.by_riot_id.insert(rid.to_lowercase(), id);
            }
        }
        Ok(r)
    }

    /// Resolve in order of how stable the identifier is: puuid never changes,
    /// a Riot ID rarely does, a display name is a guess.
    fn resolve(&self, who: &PlayerRef) -> Option<i64> {
        match who {
            PlayerRef::Puuid(p) => self.by_puuid.get(&p.to_lowercase()).copied(),
            PlayerRef::RiotId(r) => self
                .by_riot_id
                .get(&r.to_lowercase())
                .copied()
                .or_else(|| {
                    // "Vex#EUW" should still find a roster entry named "Vex".
                    r.split_once('#')
                        .and_then(|(name, _)| self.by_name.get(&name.to_lowercase()).copied())
                }),
            PlayerRef::Name(n) => self.by_name.get(&n.to_lowercase()).copied(),
        }
    }

    fn insert(&mut self, id: i64, name: String, riot_id: Option<&str>, puuid: Option<&str>) {
        self.by_name.insert(name.to_lowercase(), id);
        self.by_id.insert(id, name);
        if let Some(p) = puuid {
            self.by_puuid.insert(p.to_lowercase(), id);
        }
        if let Some(r) = riot_id {
            self.by_riot_id.insert(r.to_lowercase(), id);
        }
    }
}

/// Write (or, in a dry run, describe) a batch of matches.
pub async fn apply(
    pool: &SqlitePool,
    batch: ImportBatch,
    opts: &ImportOptions,
) -> AppResult<ImportReport> {
    let mut report = ImportReport { dry_run: opts.dry_run, ..Default::default() };
    let mut roster = Roster::load(pool).await?;

    // Everything runs in one transaction, so a failure halfway leaves no
    // half-imported history behind. A dry run rolls back at the end.
    let mut tx = pool.begin().await?;

    // Roster first, so scoreboard rows attach to players who already carry
    // their role and rank rather than to bare names created on the fly.
    if opts.create_missing_players {
        for hint in &batch.players {
            let name = hint.name.trim();
            if name.is_empty() {
                continue;
            }
            let known = roster
                .resolve(&PlayerRef::Name(name.to_string()))
                .or_else(|| hint.puuid.clone().and_then(|p| roster.resolve(&PlayerRef::Puuid(p))))
                .or_else(|| {
                    hint.riot_id.clone().and_then(|r| roster.resolve(&PlayerRef::RiotId(r)))
                });
            if known.is_some() {
                // Already on the roster — their current details win.
                continue;
            }
            report.created_players.push(name.to_string());
            if opts.dry_run {
                let placeholder = -(report.created_players.len() as i64);
                roster.insert(placeholder, name.to_string(), hint.riot_id.as_deref(), hint.puuid.as_deref());
                continue;
            }
            let row: (i64,) = sqlx::query_as(
                "INSERT INTO players (name, riot_id, role, rank, active, puuid) \
                 VALUES (?, ?, ?, ?, ?, ?) RETURNING id",
            )
            .bind(name)
            .bind(hint.riot_id.as_deref())
            .bind(hint.role.trim())
            .bind(hint.rank.as_deref())
            .bind(hint.active)
            .bind(hint.puuid.as_deref())
            .fetch_one(&mut *tx)
            .await?;
            roster.insert(row.0, name.to_string(), hint.riot_id.as_deref(), hint.puuid.as_deref());
        }
    }

    for incoming in batch.matches {
        if incoming.map.trim().is_empty() {
            report.warnings.push("Skipped a match with no map".into());
            continue;
        }

        // --- duplicate detection ---------------------------------------
        if let Some(ext) = &incoming.external_id {
            let existing: Option<(i64,)> =
                sqlx::query_as("SELECT id FROM matches WHERE external_id = ?")
                    .bind(ext)
                    .fetch_optional(&mut *tx)
                    .await?;
            if existing.is_some() {
                report.skipped_duplicates += 1;
                report.preview.push(preview(&incoming, "duplicate"));
                continue;
            }
        } else if opts.skip_existing {
            let existing: Option<(i64,)> = sqlx::query_as(
                "SELECT id FROM matches WHERE played_at = ? AND map = ? \
                 AND rounds_won = ? AND rounds_lost = ?",
            )
            .bind(&incoming.played_at)
            .bind(&incoming.map)
            .bind(incoming.rounds_won)
            .bind(incoming.rounds_lost)
            .fetch_optional(&mut *tx)
            .await?;
            if existing.is_some() {
                report.skipped_duplicates += 1;
                report.preview.push(preview(&incoming, "duplicate"));
                continue;
            }
        }

        // --- resolve the scoreboard before writing anything -------------
        let mut resolved: Vec<(i64, &IncomingPerformance)> = Vec::new();
        for perf in &incoming.performances {
            if let Some(id) = roster.resolve(&perf.player) {
                resolved.push((id, perf));
                continue;
            }
            if !opts.create_missing_players {
                let label = perf.player.label();
                if !report.unknown_players.contains(&label) {
                    report.unknown_players.push(label);
                }
                continue;
            }

            let name = match &perf.player {
                PlayerRef::Name(n) => n.clone(),
                PlayerRef::RiotId(r) => r.split('#').next().unwrap_or(r).to_string(),
                other => other.label(),
            };
            let display = unique_name(&roster, &name);
            if opts.dry_run {
                // Nothing is written, but the preview still needs a plausible
                // id, and the roster index must know the name is taken so a
                // second row for the same player does not report twice.
                let placeholder = -(report.created_players.len() as i64 + 1);
                roster.insert(placeholder, display.clone(), perf.riot_id.as_deref(), perf.puuid.as_deref());
                report.created_players.push(display);
                resolved.push((placeholder, perf));
            } else {
                let row: (i64,) = sqlx::query_as(
                    "INSERT INTO players (name, riot_id, role, rank, active, puuid) \
                     VALUES (?, ?, 'Flex', NULL, 1, ?) RETURNING id",
                )
                .bind(&display)
                .bind(perf.riot_id.as_deref())
                .bind(perf.puuid.as_deref())
                .fetch_one(&mut *tx)
                .await?;
                roster.insert(row.0, display.clone(), perf.riot_id.as_deref(), perf.puuid.as_deref());
                report.created_players.push(display);
                resolved.push((row.0, perf));
            }
        }

        if resolved.is_empty() {
            // A match nobody on the roster played tells us nothing.
            report.skipped_empty += 1;
            report.preview.push(preview(&incoming, "no known players"));
            continue;
        }

        report.preview.push(preview(&incoming, if opts.dry_run { "will import" } else { "imported" }));
        report.imported += 1;
        report.performances += resolved.len();

        if opts.dry_run {
            continue;
        }

        let match_id: (i64,) = sqlx::query_as(
            "INSERT INTO matches (played_at, map, mode, rounds_won, rounds_lost, notes, source, external_id) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?) RETURNING id",
        )
        .bind(&incoming.played_at)
        .bind(&incoming.map)
        .bind(&incoming.mode)
        .bind(incoming.rounds_won)
        .bind(incoming.rounds_lost)
        .bind(incoming.notes.as_deref())
        .bind(&incoming.source)
        .bind(incoming.external_id.as_deref())
        .fetch_one(&mut *tx)
        .await?;

        for (player_id, perf) in resolved {
            sqlx::query(
                "INSERT OR IGNORE INTO performances (match_id, player_id, agent, kills, deaths, \
                 assists, acs, first_bloods, first_deaths, plants, defuses) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(match_id.0)
            .bind(player_id)
            .bind(perf.agent.trim())
            .bind(perf.kills.max(0))
            .bind(perf.deaths.max(0))
            .bind(perf.assists.max(0))
            .bind(perf.acs.max(0))
            .bind(perf.first_bloods.max(0))
            .bind(perf.first_deaths.max(0))
            .bind(perf.plants.max(0))
            .bind(perf.defuses.max(0))
            .execute(&mut *tx)
            .await?;
        }
    }

    if opts.dry_run {
        tx.rollback().await?;
    } else {
        tx.commit().await?;
    }
    Ok(report)
}

fn preview(m: &IncomingMatch, status: &'static str) -> PreviewRow {
    PreviewRow {
        played_at: m.played_at.clone(),
        map: m.map.clone(),
        mode: m.mode.clone(),
        score: format!("{}–{}", m.rounds_won, m.rounds_lost),
        result: result_of(m.rounds_won, m.rounds_lost),
        players: m.performances.len(),
        status,
    }
}

/// `players.name` is unique, so a new arrival who collides gets a suffix
/// rather than failing the whole import.
fn unique_name(roster: &Roster, wanted: &str) -> String {
    let base = if wanted.trim().is_empty() { "Unknown" } else { wanted.trim() };
    if !roster.by_name.contains_key(&base.to_lowercase()) {
        return base.to_string();
    }
    for n in 2..100 {
        let candidate = format!("{base} ({n})");
        if !roster.by_name.contains_key(&candidate.to_lowercase()) {
            return candidate;
        }
    }
    format!("{base} ({})", uuid_like())
}

fn uuid_like() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let n = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(0);
    format!("{n:x}")
}

// ---------------------------------------------------------------------------
// JSON: the shape `/api/export` emits, so a backup restores
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct JsonBundle {
    #[serde(default)]
    pub players: Vec<PlayerInput>,
    pub matches: Vec<JsonMatch>,
}

#[derive(Debug, Deserialize)]
pub struct JsonMatch {
    pub played_at: String,
    pub map: String,
    #[serde(default = "default_mode")]
    pub mode: String,
    pub rounds_won: i64,
    pub rounds_lost: i64,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub external_id: Option<String>,
    #[serde(default)]
    pub performances: Vec<JsonPerformance>,
}

fn default_mode() -> String {
    "Competitive".to_string()
}

#[derive(Debug, Deserialize)]
pub struct JsonPerformance {
    /// Any one of these identifies the player; `player_name` is what our own
    /// export writes.
    #[serde(default)]
    pub player_name: Option<String>,
    #[serde(default)]
    pub player: Option<String>,
    #[serde(default)]
    pub riot_id: Option<String>,
    #[serde(default)]
    pub puuid: Option<String>,
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

/// Turn an export bundle into the normalised form.
///
/// Note that `player_id` from the export is deliberately ignored: restoring
/// into a database that already has a roster would otherwise attach
/// scoreboards to whoever happens to hold that id now.
pub fn from_json(bundle: JsonBundle) -> AppResult<ImportBatch> {
    if bundle.matches.is_empty() {
        return Err(AppError::BadRequest("That file has no matches in it".into()));
    }
    let players = bundle.players;
    let matches = bundle
        .matches
        .into_iter()
        .map(|m| IncomingMatch {
            external_id: m.external_id,
            source: "import".into(),
            played_at: m.played_at,
            map: m.map,
            mode: m.mode,
            rounds_won: m.rounds_won,
            rounds_lost: m.rounds_lost,
            notes: m.notes,
            performances: m
                .performances
                .into_iter()
                .map(|p| {
                    let who = p
                        .puuid
                        .clone()
                        .map(PlayerRef::Puuid)
                        .or_else(|| p.riot_id.clone().map(PlayerRef::RiotId))
                        .or_else(|| p.player_name.clone().map(PlayerRef::Name))
                        .or_else(|| p.player.clone().map(PlayerRef::Name))
                        .unwrap_or_else(|| PlayerRef::Name("Unknown".into()));
                    IncomingPerformance {
                        player: who,
                        riot_id: p.riot_id,
                        puuid: p.puuid,
                        agent: p.agent,
                        kills: p.kills,
                        deaths: p.deaths,
                        assists: p.assists,
                        acs: p.acs,
                        first_bloods: p.first_bloods,
                        first_deaths: p.first_deaths,
                        plants: p.plants,
                        defuses: p.defuses,
                    }
                })
                .collect(),
        })
        .collect();
    Ok(ImportBatch { players, matches })
}

// ---------------------------------------------------------------------------
// CSV: one row per player per match, the shape a spreadsheet produces
// ---------------------------------------------------------------------------

/// Split one CSV line, honouring quoted fields and doubled quotes inside them.
fn split_csv_line(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut field = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if in_quotes && chars.peek() == Some(&'"') => {
                field.push('"');
                chars.next();
            }
            '"' => in_quotes = !in_quotes,
            ',' if !in_quotes => out.push(std::mem::take(&mut field)),
            _ => field.push(c),
        }
    }
    out.push(field);
    out.into_iter().map(|f| f.trim().to_string()).collect()
}

pub const CSV_TEMPLATE: &str = "played_at,map,mode,rounds_won,rounds_lost,player,agent,kills,deaths,assists,acs,first_bloods,first_deaths,plants,defuses,notes";

/// Parse a CSV export into matches.
///
/// Rows are grouped into a match by (played_at, map): a five-stack's scoreboard
/// is five consecutive rows sharing those two columns.
pub fn from_csv(body: &str) -> AppResult<ImportBatch> {
    let mut lines = body.lines().filter(|l| !l.trim().is_empty());
    let header = lines
        .next()
        .ok_or_else(|| AppError::BadRequest("That CSV is empty".into()))?;
    let cols: Vec<String> = split_csv_line(header).into_iter().map(|c| c.to_lowercase()).collect();

    let index = |name: &str| cols.iter().position(|c| c == name);
    let (Some(i_date), Some(i_map), Some(i_player), Some(i_agent)) =
        (index("played_at"), index("map"), index("player"), index("agent"))
    else {
        return Err(AppError::BadRequest(format!(
            "CSV needs at least the columns played_at, map, player and agent. Expected header:\n{CSV_TEMPLATE}"
        )));
    };

    let num = |row: &[String], at: Option<usize>| -> i64 {
        at.and_then(|i| row.get(i))
            .and_then(|v| v.parse::<f64>().ok())
            .map(|v| v.round() as i64)
            .unwrap_or(0)
    };
    let text = |row: &[String], at: Option<usize>| -> Option<String> {
        at.and_then(|i| row.get(i)).map(|s| s.to_string()).filter(|s| !s.is_empty())
    };

    let (i_mode, i_won, i_lost) = (index("mode"), index("rounds_won"), index("rounds_lost"));
    let (i_k, i_d, i_a, i_acs) = (index("kills"), index("deaths"), index("assists"), index("acs"));
    let (i_fb, i_fd) = (index("first_bloods"), index("first_deaths"));
    let (i_plants, i_defuses, i_notes) = (index("plants"), index("defuses"), index("notes"));

    // Keyed by (played_at, map) and kept in first-seen order.
    let mut order: Vec<(String, String)> = Vec::new();
    let mut grouped: HashMap<(String, String), IncomingMatch> = HashMap::new();

    for (n, line) in lines.enumerate() {
        let row = split_csv_line(line);
        let Some(date) = row.get(i_date).filter(|s| !s.is_empty()) else {
            return Err(AppError::BadRequest(format!("Row {} has no played_at", n + 2)));
        };
        let Some(map) = row.get(i_map).filter(|s| !s.is_empty()) else {
            return Err(AppError::BadRequest(format!("Row {} has no map", n + 2)));
        };
        let key = (date.clone(), map.clone());

        let entry = grouped.entry(key.clone()).or_insert_with(|| {
            order.push(key.clone());
            IncomingMatch {
                external_id: None,
                source: "import".into(),
                played_at: date.clone(),
                map: map.clone(),
                mode: text(&row, i_mode).unwrap_or_else(|| "Competitive".into()),
                rounds_won: num(&row, i_won),
                rounds_lost: num(&row, i_lost),
                notes: text(&row, i_notes),
                performances: Vec::new(),
            }
        });

        let player = row.get(i_player).cloned().unwrap_or_default();
        if player.is_empty() {
            continue;
        }
        let riot_id = player.contains('#').then(|| player.clone());
        entry.performances.push(IncomingPerformance {
            player: if player.contains('#') {
                PlayerRef::RiotId(player.clone())
            } else {
                PlayerRef::Name(player.clone())
            },
            riot_id,
            puuid: None,
            agent: row.get(i_agent).cloned().unwrap_or_default(),
            kills: num(&row, i_k),
            deaths: num(&row, i_d),
            assists: num(&row, i_a),
            acs: num(&row, i_acs),
            first_bloods: num(&row, i_fb),
            first_deaths: num(&row, i_fd),
            plants: num(&row, i_plants),
            defuses: num(&row, i_defuses),
        });
    }

    if order.is_empty() {
        return Err(AppError::BadRequest("That CSV has a header but no rows".into()));
    }
    Ok(ImportBatch::matches_only(
        order.into_iter().filter_map(|k| grouped.remove(&k)).collect(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_line_handles_quotes_and_commas() {
        let row = split_csv_line(r#"2026-09-15,Ascent,"Threw a 10-3 lead, badly",13"#);
        assert_eq!(row, vec!["2026-09-15", "Ascent", "Threw a 10-3 lead, badly", "13"]);
    }

    #[test]
    fn csv_line_handles_escaped_quotes() {
        let row = split_csv_line(r#"a,"he said ""go""",c"#);
        assert_eq!(row, vec!["a", r#"he said "go""#, "c"]);
    }

    #[test]
    fn csv_groups_rows_into_matches() {
        let csv = "played_at,map,rounds_won,rounds_lost,player,agent,kills,deaths\n\
                   2026-09-15T20:00:00,Ascent,13,7,Vex,Jett,22,14\n\
                   2026-09-15T20:00:00,Ascent,13,7,Nyx,Omen,15,16\n\
                   2026-09-15T21:00:00,Lotus,9,13,Vex,Raze,18,18\n";
        let matches = from_csv(csv).unwrap().matches;
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].map, "Ascent");
        assert_eq!(matches[0].performances.len(), 2);
        assert_eq!(matches[0].rounds_won, 13);
        assert_eq!(matches[1].map, "Lotus");
        assert_eq!(matches[1].performances.len(), 1);
    }

    #[test]
    fn csv_rejects_a_missing_required_column() {
        let err = from_csv("played_at,map,kills\n2026-09-15,Ascent,20\n").unwrap_err();
        assert!(matches!(err, AppError::BadRequest(_)));
    }

    #[test]
    fn csv_accepts_a_riot_id_as_the_player() {
        let csv = "played_at,map,player,agent\n2026-09-15,Bind,Vex#EUW,Raze\n";
        let matches = from_csv(csv).unwrap().matches;
        assert_eq!(matches[0].performances[0].player, PlayerRef::RiotId("Vex#EUW".into()));
        assert_eq!(matches[0].performances[0].riot_id.as_deref(), Some("Vex#EUW"));
    }

    #[test]
    fn json_import_ignores_source_player_ids() {
        // Restoring into a populated database must not bind scoreboards to
        // whichever player happens to hold that id now.
        let bundle: JsonBundle = serde_json::from_str(
            r#"{"matches":[{"played_at":"2026-09-15","map":"Haven","rounds_won":13,
                 "rounds_lost":5,"performances":[{"player_id":99,"player_name":"Vex","agent":"Jett"}]}]}"#,
        )
        .unwrap();
        let out = from_json(bundle).unwrap().matches;
        assert_eq!(out[0].performances[0].player, PlayerRef::Name("Vex".into()));
    }
}
