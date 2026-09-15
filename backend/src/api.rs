//! HTTP surface. Everything the SPA needs, and nothing it doesn't.

use std::collections::HashMap;

use axum::extract::{Path, Query, State};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::SqlitePool;

use crate::analytics::{self, Dataset, StatsFilter};
use crate::builder::{self, BuilderQuery};
use crate::catalog;
use crate::error::{AppError, AppResult};
use crate::models::*;
use crate::seed;

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/catalog", get(get_catalog))
        .route("/players", get(list_players).post(create_player))
        .route(
            "/players/{id}",
            get(get_player).patch(update_player).delete(delete_player),
        )
        .route("/matches", get(list_matches).post(create_match))
        .route("/matches/{id}", get(get_match).delete(delete_match))
        .route("/stats/overview", get(stats_overview))
        .route("/stats/maps", get(stats_maps))
        .route("/stats/agents", get(stats_agents))
        .route("/stats/players", get(stats_players))
        .route("/stats/comps", get(stats_comps))
        .route("/builder", get(get_builder))
        .route("/export", get(export_all))
        .route("/demo/seed", post(seed_demo))
        .route("/demo/reset", delete(reset_demo))
        .with_state(state)
}

async fn health() -> Json<Value> {
    Json(json!({ "status": "ok", "version": env!("CARGO_PKG_VERSION") }))
}

async fn get_catalog() -> Json<Value> {
    Json(json!({
        "agents": catalog::AGENTS,
        "maps": catalog::MAPS,
        "roles": catalog::ROLES,
    }))
}

// ---------------------------------------------------------------------------
// Players
// ---------------------------------------------------------------------------

const PLAYER_COLUMNS: &str = "id, name, riot_id, role, rank, active, created_at";

async fn list_players(State(st): State<AppState>) -> AppResult<Json<Vec<Player>>> {
    let rows: Vec<Player> = sqlx::query_as(&format!(
        "SELECT {PLAYER_COLUMNS} FROM players ORDER BY active DESC, name"
    ))
    .fetch_all(&st.pool)
    .await?;
    Ok(Json(rows))
}

async fn get_player(
    State(st): State<AppState>,
    Path(id): Path<i64>,
) -> AppResult<Json<analytics::PlayerStat>> {
    let ds = Dataset::load(&st.pool, &StatsFilter::default()).await?;
    analytics::player_stats(&ds)
        .into_iter()
        .find(|p| p.player_id == id)
        .map(Json)
        .ok_or_else(|| AppError::NotFound(format!("No player with id {id}")))
}

async fn create_player(
    State(st): State<AppState>,
    Json(input): Json<PlayerInput>,
) -> AppResult<Json<Player>> {
    let name = input.name.trim();
    if name.is_empty() {
        return Err(AppError::BadRequest("A player needs a name".into()));
    }
    let row: Player = sqlx::query_as(&format!(
        "INSERT INTO players (name, riot_id, role, rank, active) VALUES (?, ?, ?, ?, ?) \
         RETURNING {PLAYER_COLUMNS}"
    ))
    .bind(name)
    .bind(input.riot_id.as_deref().map(str::trim).filter(|s| !s.is_empty()))
    .bind(input.role.trim())
    .bind(input.rank.as_deref().map(str::trim).filter(|s| !s.is_empty()))
    .bind(input.active)
    .fetch_one(&st.pool)
    .await?;
    Ok(Json(row))
}

async fn update_player(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    Json(input): Json<PlayerInput>,
) -> AppResult<Json<Player>> {
    let row: Option<Player> = sqlx::query_as(&format!(
        "UPDATE players SET name = ?, riot_id = ?, role = ?, rank = ?, active = ? \
         WHERE id = ? RETURNING {PLAYER_COLUMNS}"
    ))
    .bind(input.name.trim())
    .bind(input.riot_id.as_deref().map(str::trim).filter(|s| !s.is_empty()))
    .bind(input.role.trim())
    .bind(input.rank.as_deref().map(str::trim).filter(|s| !s.is_empty()))
    .bind(input.active)
    .bind(id)
    .fetch_optional(&st.pool)
    .await?;
    row.map(Json)
        .ok_or_else(|| AppError::NotFound(format!("No player with id {id}")))
}

async fn delete_player(State(st): State<AppState>, Path(id): Path<i64>) -> AppResult<Json<Value>> {
    let res = sqlx::query("DELETE FROM players WHERE id = ?")
        .bind(id)
        .execute(&st.pool)
        .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("No player with id {id}")));
    }
    Ok(Json(json!({ "deleted": id })))
}

// ---------------------------------------------------------------------------
// Matches
// ---------------------------------------------------------------------------

const MATCH_COLUMNS: &str = "id, played_at, map, mode, rounds_won, rounds_lost, notes";

#[derive(Debug, Deserialize)]
struct MatchListQuery {
    #[serde(default)]
    limit: Option<i64>,
    #[serde(default)]
    map: Option<String>,
}

async fn list_matches(
    State(st): State<AppState>,
    Query(q): Query<MatchListQuery>,
) -> AppResult<Json<Vec<MatchDetail>>> {
    let limit = q.limit.unwrap_or(200).clamp(1, 2000);
    let mut sql = format!("SELECT {MATCH_COLUMNS} FROM matches");
    if q.map.is_some() {
        sql.push_str(" WHERE map = ?");
    }
    sql.push_str(" ORDER BY played_at DESC, id DESC LIMIT ?");

    let mut query = sqlx::query_as::<_, MatchRow>(&sql);
    if let Some(map) = &q.map {
        query = query.bind(map.clone());
    }
    let matches: Vec<MatchRow> = query.bind(limit).fetch_all(&st.pool).await?;

    Ok(Json(hydrate(&st.pool, matches).await?))
}

async fn get_match(State(st): State<AppState>, Path(id): Path<i64>) -> AppResult<Json<MatchDetail>> {
    let row: Option<MatchRow> = sqlx::query_as(&format!("SELECT {MATCH_COLUMNS} FROM matches WHERE id = ?"))
        .bind(id)
        .fetch_optional(&st.pool)
        .await?;
    let row = row.ok_or_else(|| AppError::NotFound(format!("No match with id {id}")))?;
    let mut detail = hydrate(&st.pool, vec![row]).await?;
    Ok(Json(detail.remove(0)))
}

/// Attach scoreboards and player names to a list of matches.
async fn hydrate(pool: &SqlitePool, matches: Vec<MatchRow>) -> AppResult<Vec<MatchDetail>> {
    let ids: Vec<i64> = matches.iter().map(|m| m.id).collect();
    let perfs = analytics::load_performances(pool, &ids).await?;
    let players: Vec<Player> = sqlx::query_as(&format!("SELECT {PLAYER_COLUMNS} FROM players"))
        .fetch_all(pool)
        .await?;
    let names: HashMap<i64, String> = players.into_iter().map(|p| (p.id, p.name)).collect();

    let mut by_match: HashMap<i64, Vec<PerformanceDetail>> = HashMap::new();
    for row in perfs {
        let player_name = names
            .get(&row.player_id)
            .cloned()
            .unwrap_or_else(|| "Unknown".to_string());
        let agent_role = catalog::role_of(&row.agent);
        by_match
            .entry(row.match_id)
            .or_default()
            .push(PerformanceDetail { row, player_name, agent_role });
    }
    for list in by_match.values_mut() {
        list.sort_by(|a, b| b.row.acs.cmp(&a.row.acs));
    }

    Ok(matches
        .into_iter()
        .map(|info| MatchDetail {
            result: result_of(info.rounds_won, info.rounds_lost),
            performances: by_match.remove(&info.id).unwrap_or_default(),
            info,
        })
        .collect())
}

async fn create_match(
    State(st): State<AppState>,
    Json(input): Json<MatchInput>,
) -> AppResult<Json<MatchDetail>> {
    if input.map.trim().is_empty() {
        return Err(AppError::BadRequest("Pick a map".into()));
    }
    if input.rounds_won < 0 || input.rounds_lost < 0 {
        return Err(AppError::BadRequest("Round counts cannot be negative".into()));
    }
    if input.rounds_won == 0 && input.rounds_lost == 0 {
        return Err(AppError::BadRequest("Enter the final score".into()));
    }

    let mut tx = st.pool.begin().await?;
    let (match_id,): (i64,) = sqlx::query_as(
        "INSERT INTO matches (played_at, map, mode, rounds_won, rounds_lost, notes) \
         VALUES (?, ?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(input.played_at.trim())
    .bind(input.map.trim())
    .bind(input.mode.trim())
    .bind(input.rounds_won)
    .bind(input.rounds_lost)
    .bind(input.notes.as_deref().map(str::trim).filter(|s| !s.is_empty()))
    .fetch_one(&mut *tx)
    .await?;

    for p in &input.performances {
        if p.agent.trim().is_empty() {
            continue;
        }
        sqlx::query(
            "INSERT INTO performances (match_id, player_id, agent, kills, deaths, assists, acs, \
             first_bloods, first_deaths, plants, defuses) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(match_id)
        .bind(p.player_id)
        .bind(p.agent.trim())
        .bind(p.kills.max(0))
        .bind(p.deaths.max(0))
        .bind(p.assists.max(0))
        .bind(p.acs.max(0))
        .bind(p.first_bloods.max(0))
        .bind(p.first_deaths.max(0))
        .bind(p.plants.max(0))
        .bind(p.defuses.max(0))
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;

    get_match(State(st), Path(match_id)).await
}

async fn delete_match(State(st): State<AppState>, Path(id): Path<i64>) -> AppResult<Json<Value>> {
    let res = sqlx::query("DELETE FROM matches WHERE id = ?")
        .bind(id)
        .execute(&st.pool)
        .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("No match with id {id}")));
    }
    Ok(Json(json!({ "deleted": id })))
}

// ---------------------------------------------------------------------------
// Stats
// ---------------------------------------------------------------------------

async fn stats_overview(
    State(st): State<AppState>,
    Query(filter): Query<StatsFilter>,
) -> AppResult<Json<analytics::Overview>> {
    let ds = Dataset::load(&st.pool, &filter).await?;
    Ok(Json(analytics::overview(&ds)))
}

async fn stats_maps(
    State(st): State<AppState>,
    Query(filter): Query<StatsFilter>,
) -> AppResult<Json<Vec<analytics::MapStat>>> {
    let ds = Dataset::load(&st.pool, &filter).await?;
    Ok(Json(analytics::map_stats(&ds)))
}

async fn stats_agents(
    State(st): State<AppState>,
    Query(filter): Query<StatsFilter>,
) -> AppResult<Json<Vec<analytics::AgentStat>>> {
    let ds = Dataset::load(&st.pool, &filter).await?;
    Ok(Json(analytics::agent_stats(&ds)))
}

async fn stats_players(
    State(st): State<AppState>,
    Query(filter): Query<StatsFilter>,
) -> AppResult<Json<Vec<analytics::PlayerStat>>> {
    let ds = Dataset::load(&st.pool, &filter).await?;
    Ok(Json(analytics::player_stats(&ds)))
}

async fn stats_comps(
    State(st): State<AppState>,
    Query(filter): Query<StatsFilter>,
) -> AppResult<Json<Vec<analytics::CompStat>>> {
    let ds = Dataset::load(&st.pool, &filter).await?;
    Ok(Json(analytics::comp_stats(&ds)))
}

async fn get_builder(
    State(st): State<AppState>,
    Query(q): Query<BuilderQuery>,
) -> AppResult<Json<builder::BuilderResult>> {
    if q.map.trim().is_empty() {
        return Err(AppError::BadRequest("Choose a map to build for".into()));
    }
    let ds = Dataset::load(&st.pool, &StatsFilter::default()).await?;
    Ok(Json(builder::build(&ds, &q)))
}

// ---------------------------------------------------------------------------
// Data management
// ---------------------------------------------------------------------------

async fn export_all(State(st): State<AppState>) -> AppResult<Json<Value>> {
    let players: Vec<Player> = sqlx::query_as(&format!("SELECT {PLAYER_COLUMNS} FROM players"))
        .fetch_all(&st.pool)
        .await?;
    let matches: Vec<MatchRow> =
        sqlx::query_as(&format!("SELECT {MATCH_COLUMNS} FROM matches ORDER BY played_at"))
            .fetch_all(&st.pool)
            .await?;
    let detail = hydrate(&st.pool, matches).await?;
    Ok(Json(json!({
        "exported_at": chrono::Utc::now().to_rfc3339(),
        "players": players,
        "matches": detail,
    })))
}

#[derive(Debug, Deserialize)]
struct SeedQuery {
    /// Number of play sessions to generate (2-4 matches each).
    #[serde(default)]
    sessions: Option<usize>,
}

async fn seed_demo(
    State(st): State<AppState>,
    Query(q): Query<SeedQuery>,
) -> AppResult<Json<Value>> {
    seed::wipe(&st.pool).await?;
    let sessions = q.sessions.unwrap_or(45).clamp(1, 400);
    let (players, matches) = seed::generate(&st.pool, sessions).await?;
    Ok(Json(json!({ "players": players, "matches": matches })))
}

async fn reset_demo(State(st): State<AppState>) -> AppResult<Json<Value>> {
    seed::wipe(&st.pool).await?;
    Ok(Json(json!({ "reset": true })))
}
