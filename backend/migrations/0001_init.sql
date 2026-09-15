CREATE TABLE IF NOT EXISTS players (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    name        TEXT    NOT NULL UNIQUE,
    riot_id     TEXT,
    role        TEXT    NOT NULL DEFAULT 'Flex',
    rank        TEXT,
    active      BOOLEAN NOT NULL DEFAULT 1,
    created_at  TEXT    NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS matches (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    played_at   TEXT    NOT NULL,
    map         TEXT    NOT NULL,
    mode        TEXT    NOT NULL DEFAULT 'Competitive',
    rounds_won  INTEGER NOT NULL,
    rounds_lost INTEGER NOT NULL,
    notes       TEXT,
    created_at  TEXT    NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS performances (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    match_id     INTEGER NOT NULL REFERENCES matches(id) ON DELETE CASCADE,
    player_id    INTEGER NOT NULL REFERENCES players(id) ON DELETE CASCADE,
    agent        TEXT    NOT NULL,
    kills        INTEGER NOT NULL DEFAULT 0,
    deaths       INTEGER NOT NULL DEFAULT 0,
    assists      INTEGER NOT NULL DEFAULT 0,
    acs          INTEGER NOT NULL DEFAULT 0,
    first_bloods INTEGER NOT NULL DEFAULT 0,
    first_deaths INTEGER NOT NULL DEFAULT 0,
    plants       INTEGER NOT NULL DEFAULT 0,
    defuses      INTEGER NOT NULL DEFAULT 0,
    UNIQUE (match_id, player_id)
);

CREATE INDEX IF NOT EXISTS idx_matches_played_at ON matches (played_at DESC);
CREATE INDEX IF NOT EXISTS idx_perf_match ON performances (match_id);
CREATE INDEX IF NOT EXISTS idx_perf_player ON performances (player_id);
