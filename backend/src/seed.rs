//! Deterministic demo data.
//!
//! A brand-new install with an empty database has nothing to chart, so the app
//! can seed a plausible six months of five-stack history. The generator is fully
//! deterministic (fixed-seed xorshift) so the numbers are stable across restarts
//! and everyone on the team sees the same demo.

use chrono::{Duration, Utc};
use sqlx::SqlitePool;

use crate::error::AppResult;

const SEED: u64 = 0x5641_4c4f_5241_4e54; // "VALORANT"

struct Rng(u64);

impl Rng {
    fn next_u64(&mut self) -> u64 {
        // SplitMix64. Chosen over a bare xorshift because the generator draws a
        // near-constant number of values per simulated match, and xorshift's
        // lattice structure turned that fixed stride into visible correlation
        // between which map was picked and whether the match was won.
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// Uniform in [0, 1).
    fn f(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
    fn range(&mut self, lo: i64, hi: i64) -> i64 {
        if hi <= lo {
            return lo;
        }
        lo + (self.next_u64() % (hi - lo) as u64) as i64
    }
    /// Roughly normal via the mean of three uniforms.
    fn jitter(&mut self, spread: f64) -> f64 {
        ((self.f() + self.f() + self.f()) / 3.0 - 0.5) * 2.0 * spread
    }
    fn weighted<'a, T>(&mut self, items: &'a [(T, f64)]) -> &'a T {
        let total: f64 = items.iter().map(|(_, w)| w).sum();
        let mut roll = self.f() * total;
        for (item, weight) in items {
            roll -= weight;
            if roll <= 0.0 {
                return item;
            }
        }
        &items[items.len() - 1].0
    }
}

struct Profile {
    name: &'static str,
    riot_id: &'static str,
    role: &'static str,
    rank: &'static str,
    /// 0-1, drives frag output.
    skill: f64,
    /// 0-1, drives entry attempts (first bloods *and* first deaths).
    aggression: f64,
    /// Agents this player picks, with weights. Per-map overrides come first.
    pool: &'static [(&'static str, f64)],
    map_pool: &'static [(&'static str, &'static [(&'static str, f64)])],
}

const ROSTER: &[Profile] = &[
    Profile {
        name: "Vex",
        riot_id: "Vex#EUW",
        role: "Duelist",
        rank: "Ascendant 2",
        skill: 0.86,
        aggression: 0.9,
        pool: &[("Jett", 5.0), ("Raze", 3.0), ("Reyna", 1.0), ("Neon", 1.0)],
        map_pool: &[
            ("Split", &[("Raze", 6.0), ("Jett", 1.0), ("Neon", 2.0)]),
            ("Bind", &[("Raze", 5.0), ("Yoru", 1.5)]),
            ("Icebox", &[("Jett", 6.0), ("Reyna", 1.0)]),
            ("Breeze", &[("Jett", 6.0), ("Reyna", 1.0)]),
            ("Fracture", &[("Neon", 4.0), ("Raze", 3.0)]),
        ],
    },
    Profile {
        name: "Nyx",
        riot_id: "Nyx#0001",
        role: "Controller",
        rank: "Ascendant 1",
        skill: 0.62,
        aggression: 0.35,
        pool: &[("Omen", 4.0), ("Brimstone", 2.0), ("Astra", 1.0), ("Clove", 1.5)],
        map_pool: &[
            ("Breeze", &[("Viper", 6.0), ("Harbor", 2.0)]),
            ("Icebox", &[("Viper", 6.0), ("Harbor", 1.5)]),
            ("Lotus", &[("Omen", 4.0), ("Harbor", 2.0), ("Astra", 1.5)]),
            ("Abyss", &[("Clove", 4.0), ("Omen", 3.0)]),
            ("Corrode", &[("Omen", 3.0), ("Brimstone", 3.0)]),
        ],
    },
    Profile {
        name: "Roach",
        riot_id: "Roach#NA1",
        role: "Initiator",
        rank: "Diamond 3",
        skill: 0.7,
        aggression: 0.5,
        pool: &[("Sova", 4.0), ("Fade", 3.0), ("KAY/O", 2.0), ("Gekko", 1.5)],
        map_pool: &[
            ("Ascent", &[("Sova", 7.0), ("KAY/O", 1.5)]),
            ("Split", &[("Fade", 5.0), ("Skye", 1.5)]),
            ("Lotus", &[("Fade", 4.0), ("Sova", 3.0)]),
            ("Bind", &[("Gekko", 3.0), ("Fade", 3.0), ("Tejo", 2.0)]),
        ],
    },
    Profile {
        name: "Static",
        riot_id: "Static#GG",
        role: "Sentinel",
        rank: "Diamond 2",
        skill: 0.58,
        aggression: 0.25,
        pool: &[("Killjoy", 4.0), ("Cypher", 3.0), ("Vyse", 1.5), ("Deadlock", 1.0)],
        map_pool: &[
            ("Ascent", &[("Killjoy", 6.0), ("Cypher", 2.0)]),
            ("Haven", &[("Cypher", 5.0), ("Killjoy", 2.0)]),
            ("Bind", &[("Cypher", 4.0), ("Vyse", 2.5)]),
            ("Icebox", &[("Sage", 4.0), ("Killjoy", 3.0)]),
            ("Breeze", &[("Cypher", 4.0), ("Chamber", 2.0)]),
        ],
    },
    Profile {
        name: "Mimo",
        riot_id: "Mimo#FLEX",
        role: "Flex",
        rank: "Ascendant 1",
        skill: 0.74,
        aggression: 0.6,
        pool: &[("Skye", 3.0), ("Breach", 2.5), ("Sage", 1.5), ("Phoenix", 1.5), ("Iso", 1.0)],
        map_pool: &[
            ("Ascent", &[("Breach", 4.0), ("Phoenix", 2.0)]),
            ("Split", &[("Skye", 4.0), ("Sage", 2.5)]),
            ("Sunset", &[("Breach", 4.0), ("Skye", 2.0)]),
            ("Abyss", &[("Skye", 4.0), ("Neon", 2.0)]),
        ],
    },
];

/// (map, how often we queue it, how good we are on it)
const MAP_PROFILE: &[(&str, f64, f64)] = &[
    ("Ascent", 5.0, 0.70),
    ("Lotus", 4.0, 0.62),
    ("Bind", 3.5, 0.56),
    ("Haven", 3.5, 0.52),
    ("Split", 3.0, 0.47),
    ("Corrode", 2.5, 0.51),
    ("Abyss", 2.5, 0.45),
    ("Sunset", 2.0, 0.50),
    ("Icebox", 2.0, 0.38),
    ("Fracture", 1.5, 0.44),
    ("Pearl", 1.5, 0.48),
    ("Breeze", 1.5, 0.34),
];

pub async fn is_empty(pool: &SqlitePool) -> AppResult<bool> {
    let (players,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM players")
        .fetch_one(pool)
        .await?;
    let (matches,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM matches")
        .fetch_one(pool)
        .await?;
    Ok(players == 0 && matches == 0)
}

pub async fn wipe(pool: &SqlitePool) -> AppResult<()> {
    sqlx::query("DELETE FROM performances").execute(pool).await?;
    sqlx::query("DELETE FROM matches").execute(pool).await?;
    sqlx::query("DELETE FROM players").execute(pool).await?;
    sqlx::query("DELETE FROM sqlite_sequence WHERE name IN ('players','matches','performances')")
        .execute(pool)
        .await
        .ok();
    Ok(())
}

/// Insert the demo roster and `sessions` nights of matches. Returns (players, matches).
pub async fn generate(pool: &SqlitePool, sessions: usize) -> AppResult<(usize, usize)> {
    let mut rng = Rng(SEED);
    let mut tx = pool.begin().await?;

    let mut player_ids = Vec::new();
    for p in ROSTER {
        let id: (i64,) = sqlx::query_as(
            "INSERT INTO players (name, riot_id, role, rank, active) VALUES (?, ?, ?, ?, 1) RETURNING id",
        )
        .bind(p.name)
        .bind(p.riot_id)
        .bind(p.role)
        .bind(p.rank)
        .fetch_one(&mut *tx)
        .await?;
        player_ids.push(id.0);
    }

    let map_weights: Vec<(&str, f64)> = MAP_PROFILE.iter().map(|(m, w, _)| (*m, *w)).collect();
    let mut matches = 0usize;
    // Walk forward in time so the timeline reads left-to-right in the UI.
    let mut day_offset = (sessions as i64) * 3 + 6;

    for _ in 0..sessions {
        day_offset -= rng.range(2, 5);
        let games_tonight = rng.range(2, 5);
        for game in 0..games_tonight {
            let map = *rng.weighted(&map_weights);
            let base = MAP_PROFILE
                .iter()
                .find(|(m, _, _)| *m == map)
                .map(|(_, _, s)| *s)
                .unwrap_or(0.5);

            // Agent picks for the night's comp.
            let picks: Vec<&'static str> = ROSTER
                .iter()
                .map(|profile| {
                    let pool = profile
                        .map_pool
                        .iter()
                        .find(|(m, _)| *m == map)
                        .map(|(_, p)| *p)
                        .unwrap_or(profile.pool);
                    *rng.weighted(pool)
                })
                .collect();

            // Late-night games go slightly worse; form drifts match to match.
            let fatigue = (game as f64) * 0.02;
            let win_chance = (base - fatigue + rng.jitter(0.12)).clamp(0.08, 0.92);
            let won = rng.f() < win_chance;

            let (rounds_won, rounds_lost) = score_line(&mut rng, won, win_chance);
            let total_rounds = rounds_won + rounds_lost;

            let played_at = (Utc::now()
                - Duration::days(day_offset)
                - Duration::minutes(45 * (games_tonight - game)))
            .format("%Y-%m-%dT%H:%M:%S")
            .to_string();

            let match_id: (i64,) = sqlx::query_as(
                "INSERT INTO matches (played_at, map, mode, rounds_won, rounds_lost, notes, source) \
                 VALUES (?, ?, 'Competitive', ?, ?, NULL, 'demo') RETURNING id",
            )
            .bind(&played_at)
            .bind(map)
            .bind(rounds_won)
            .bind(rounds_lost)
            .fetch_one(&mut *tx)
            .await?;

            for (i, profile) in ROSTER.iter().enumerate() {
                let agent = picks[i];
                let stats = scoreboard(&mut rng, profile, agent, won, total_rounds);
                sqlx::query(
                    "INSERT INTO performances (match_id, player_id, agent, kills, deaths, assists, \
                     acs, first_bloods, first_deaths, plants, defuses) \
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                )
                .bind(match_id.0)
                .bind(player_ids[i])
                .bind(agent)
                .bind(stats.kills)
                .bind(stats.deaths)
                .bind(stats.assists)
                .bind(stats.acs)
                .bind(stats.first_bloods)
                .bind(stats.first_deaths)
                .bind(stats.plants)
                .bind(stats.defuses)
                .execute(&mut *tx)
                .await?;
            }
            matches += 1;
        }
    }

    tx.commit().await?;
    Ok((ROSTER.len(), matches))
}

fn score_line(rng: &mut Rng, won: bool, win_chance: f64) -> (i64, i64) {
    // Closer expected games produce closer scorelines.
    let dominance = (win_chance - 0.5).abs() * 2.0;
    let loser = if rng.f() < 0.12 {
        // Overtime.
        let ot = rng.range(0, 3) * 2;
        return if won { (14 + ot, 12 + ot) } else { (12 + ot, 14 + ot) };
    } else {
        let spread = 3.0 + (1.0 - dominance) * 7.0 + rng.jitter(3.0);
        (12.0 - spread).clamp(1.0, 11.0).round() as i64
    };
    if won {
        (13, loser)
    } else {
        (loser, 13)
    }
}

struct Line {
    kills: i64,
    deaths: i64,
    assists: i64,
    acs: i64,
    first_bloods: i64,
    first_deaths: i64,
    plants: i64,
    defuses: i64,
}

fn scoreboard(rng: &mut Rng, profile: &Profile, agent: &str, won: bool, rounds: i64) -> Line {
    let rounds_f = rounds as f64;
    // Agents a player is sharper on get a small bump — this is what makes
    // "best agent" analysis in the UI find something real.
    let signature = profile
        .map_pool
        .iter()
        .flat_map(|(_, pool)| pool.iter())
        .chain(profile.pool.iter())
        .find(|(a, _)| *a == agent)
        .map(|(_, w)| (w / 6.0).min(0.12))
        .unwrap_or(0.0);

    let form = if won { 0.09 } else { -0.07 };
    let kill_rate = (0.52 + (profile.skill - 0.5) * 0.55 + signature + form + rng.jitter(0.1))
        .clamp(0.22, 1.15);
    let death_rate = (0.78 - (profile.skill - 0.5) * 0.28 - form * 0.6 + rng.jitter(0.08))
        .clamp(0.4, 1.0);
    let assist_rate = match profile.role {
        "Controller" | "Initiator" => 0.42,
        "Sentinel" => 0.3,
        "Flex" => 0.36,
        _ => 0.18,
    } + rng.jitter(0.08);

    let kills = (rounds_f * kill_rate).round().max(1.0) as i64;
    let deaths = (rounds_f * death_rate).round().max(1.0) as i64;
    let assists = (rounds_f * assist_rate.max(0.05)).round() as i64;
    // ACS tracks damage, which tracks kills, plus a little utility credit.
    let acs = ((kills as f64 / rounds_f) * 240.0 + (assists as f64 / rounds_f) * 45.0
        + rng.jitter(25.0)
        + 25.0)
        .clamp(60.0, 420.0)
        .round() as i64;

    // Opening duels are close to zero-sum in reality, so the two rates stay
    // near each other and only skill and the game's outcome tilt them.
    let entry_attempts = rounds_f * profile.aggression * 0.26;
    let first_bloods = (entry_attempts * (0.30 + profile.skill * 0.26 + form + rng.jitter(0.12)))
        .round()
        .max(0.0) as i64;
    let first_deaths = (entry_attempts * (0.52 - profile.skill * 0.12 - form + rng.jitter(0.12)))
        .round()
        .max(0.0) as i64;

    let (plants, defuses) = match profile.role {
        "Sentinel" => (rng.range(0, 3), rng.range(0, 4)),
        "Controller" => (rng.range(1, 5), rng.range(0, 2)),
        _ => (rng.range(0, 4), rng.range(0, 2)),
    };

    Line {
        kills,
        deaths,
        assists,
        acs,
        first_bloods,
        first_deaths,
        plants,
        defuses,
    }
}
