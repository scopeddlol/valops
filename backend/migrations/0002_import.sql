-- Provenance and idempotency for imported data.

-- Where a match came from: 'manual', 'demo', 'import', or a source name.
ALTER TABLE matches ADD COLUMN source TEXT NOT NULL DEFAULT 'manual';

-- The source's own id for this match. Unique where present, so re-running an
-- import is a no-op rather than a duplicate. (A partial index lets the many
-- manually logged matches keep a NULL here without colliding.)
ALTER TABLE matches ADD COLUMN external_id TEXT;
CREATE UNIQUE INDEX IF NOT EXISTS idx_matches_external_id
    ON matches (external_id) WHERE external_id IS NOT NULL;

-- Riot's stable player id, so pulled matches attach to the right person even
-- if they change their display name.
ALTER TABLE players ADD COLUMN puuid TEXT;
CREATE UNIQUE INDEX IF NOT EXISTS idx_players_puuid
    ON players (puuid) WHERE puuid IS NOT NULL;
