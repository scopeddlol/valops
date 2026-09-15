# valops

[![Docker](https://github.com/scopeddlol/valops/actions/workflows/docker.yml/badge.svg)](https://github.com/scopeddlol/valops/actions/workflows/docker.yml)

A self-hosted stats desk for a Valorant five-stack. Log your matches, see which
maps you actually win on, and get a comp built from your own history rather than
somebody else's tier list.

One container, one SQLite file. Nothing leaves your machine, and no Riot API key
is needed.

```bash
# Run the published image
docker run -d --name valops -p 8080:8080 -v valops-data:/app/data \
  ghcr.io/scopeddlol/valops:latest

# …or build it yourself
docker compose up -d --build
```

Then open http://localhost:8080.

The first run seeds a realistic six-month demo history so there is something to
look at immediately. Wipe it from **Matches → Data → Delete all data**, or start
clean with `VALOPS_SEED_DEMO=false`.

## What it does

| Page | What you get |
|---|---|
| **Dashboard** | Record, win rate, team K/D, ACS, recent form, win rate by map, a rolling ten-match trend line, squad MVP and best agent |
| **Team Builder** | Pick a map, get five agents with the reasoning behind each, swap any pick and the rest re-solves around it |
| **Roster** | A card per player with K/D, KDA, ACS and entry differential; open one for their agent and map splits and a K/D trend |
| **Maps** | Every map with record, round differential, agent picks, plus a player × map K/D heatmap |
| **Agents** | Pick rates, win rates, a player × agent mastery grid, and the agents nobody in the stack has touched |
| **Matches** | The match log with full scoreboards, and the form for logging a new one |
| **Import** | Bring history in from a file or pull it by Riot ID, with a preview before anything is written |

## How the team builder decides

Every (player, agent) pair on the chosen map is scored from that player's own
record:

| Weight | Component |
|---|---|
| 30% | Winning — their record on that agent, on that map, backed by their record on the agent everywhere |
| 22% | Impact — ACS and K/D on that agent relative to their own baseline |
| 18% | Comfort — how many reps they have on it |
| 15% | Role fit — does it match the role they signed up for |
| 15% | Map meta — how the stack does with that agent on that map, whoever plays it |

It then searches every legal combination of the top candidates (no duplicate
agents) and picks the highest-scoring comp, with a bonus for covering
controller, initiator, sentinel and duelist and a penalty for tripling up on a
role. Each slot shows the numbers behind it, so you can disagree with it.

Two statistical details keep the output honest on small samples:

- **Win rates are smoothed** toward the team's baseline, so a 2-0 night on an
  off-agent does not read as "100%".
- **"Best of" lists rank by a Wilson lower bound**, so a 42-game main outranks a
  two-game cameo instead of the other way round.

## Configuration

Everything is an environment variable; the defaults are what compose uses.

| Variable | Default | Purpose |
|---|---|---|
| `PORT` | `8080` | Port the server listens on |
| `VALOPS_DATABASE_URL` | `sqlite:///app/data/valops.db` | Database location |
| `VALOPS_STATIC_DIR` | `/app/static` | Built frontend to serve |
| `VALOPS_SEED_DEMO` | `true` | Seed demo history when the database is empty |
| `VALOPS_SEED_SESSIONS` | `45` | How many play sessions the demo generates |
| `VALOPS_HENRIK_KEY` | *(unset)* | HenrikDev API key. Without it, file import still works; with it, the Riot ID tab is enabled |
| `RUST_LOG` | `valops=info,tower_http=warn` | Log filter |

Change the published port with `VALOPS_PORT=9000 docker compose up -d`.

## Getting data in

Four ways, all of which land in the same place:

**1. Log it by hand.** Matches → Log match. The form pre-fills each player's
most-played agent and the map you last played, so it is mostly typing the
scoreboard.

**2. A CSV from a spreadsheet.** Import → File. One row per player per match;
rows sharing a `played_at` and `map` are grouped into one match. Grab the
header from the *CSV template* button, or:

```
played_at,map,mode,rounds_won,rounds_lost,player,agent,kills,deaths,assists,acs,first_bloods,first_deaths,plants,defuses,notes
2026-09-15T21:30:00,Ascent,Competitive,13,9,Vex,Jett,22,15,4,271,6,3,1,0,
2026-09-15T21:30:00,Ascent,Competitive,13,9,Nyx,Omen,15,16,9,198,1,2,3,1,
```

Only `played_at`, `map`, `player` and `agent` are required. A `player` written
as `Name#TAG` is matched by Riot ID; anything else is matched by name.

**3. A previous export.** Import → File, or the Backup tab. This is the restore
path for the JSON that `/api/export` produces.

**4. Pull by Riot ID.** Import → Riot ID, which fetches recent matches through
the HenrikDev API and keeps the rows belonging to players on your roster. Needs
`VALOPS_HENRIK_KEY`; keys are requested on their Discord. See the caveat below.

Every route previews before it writes: run **Preview** (a dry run inside a
transaction that is rolled back), look at exactly which matches would land, then
commit. Re-running an import is safe — a match already present is skipped, not
duplicated.

### On the Riot ID source

**This source's response mapping has not been run against the live API.** It was
written from the documented endpoint and auth scheme in an environment where
`api.henrikdev.xyz` was unreachable, so the field mapping is an informed
reconstruction rather than a verified one. Everything else on this page —
CSV, JSON, dedup, preview — was tested end to end.

To make that safe rather than merely disclosed, the mapper is deliberately
tolerant (each field is looked up through several candidate paths, covering the
shapes v2 and v4 are documented to use) and it ships with a **probe**: click
*Run probe*, or

```bash
curl -X POST http://localhost:8080/api/sources/henrik/probe \
  -H 'Content-Type: application/json' \
  -d '{"name":"Vex#EUW","region":"eu"}'
```

It fetches without importing and reports which fields it resolved, which it
could not, and the keys the API actually returned — enough to turn a mismatch
into a one-line fix in `backend/src/sources/henrik.rs`.

Two known limits of this source: opening duels (first bloods and first deaths)
are not exposed at match level, so imported matches record them as zero; and
combat score is converted to ACS by dividing by rounds played.

Other sources are not available rather than unimplemented: **tracker.gg has no
public API for reading match history** (their developer API is for pushing match
data *to* them from game servers), and Riot's own Valorant match endpoints have
historically required a production key granted only to approved applications.

## Your data

Matches live in the `valops-data` volume, mounted at `/app/data`.

```bash
# Back up
docker compose exec valops cat /app/data/valops.db > valops-backup.db

# Or export everything as JSON (Import -> Backup does the same)
curl http://localhost:8080/api/export > valops-export.json

# Restore it
curl -X POST http://localhost:8080/api/import/json \
  -H 'Content-Type: application/json' \
  -d "$(jq '. + {options:{dry_run:false}}' valops-export.json)"
```

## Running without Docker

The Rust toolchain is pinned in `backend/rust-toolchain.toml`, so rustup picks
up the same compiler CI and the Docker build use. Two terminals; the Vite dev
server proxies `/api` to the Rust process.

```bash
cd backend && cargo run                  # http://127.0.0.1:8080
cd frontend && npm install && npm run dev # http://127.0.0.1:5173
```

For a production build outside Docker, `npm run build` then point the binary at
the output:

```bash
cd frontend && npm run build
cd ../backend && VALOPS_STATIC_DIR=../frontend/dist cargo run --release
```

## API

The frontend is a normal client of this API; anything it can do, you can script.

```
GET    /api/health
GET    /api/catalog                 agents (with roles) and maps

GET    /api/players                 POST /api/players
GET    /api/players/{id}            full stats for one player
PATCH  /api/players/{id}            DELETE /api/players/{id}

GET    /api/matches?limit=&map=     POST /api/matches
GET    /api/matches/{id}            DELETE /api/matches/{id}

GET    /api/stats/overview          headline numbers, form and trend
GET    /api/stats/maps              per-map breakdown
GET    /api/stats/agents            per-agent breakdown
GET    /api/stats/players           per-player breakdown
GET    /api/stats/comps             five-agent compositions and their records

GET    /api/builder?map=Ascent&players=1,2,3,4,5&locks=1:Jett&exclude=Reyna
GET    /api/export                  everything as JSON
POST   /api/import/json             restore an export (body: bundle + options)
POST   /api/import/csv              import a CSV (text/csv body, ?dry_run=true)
GET    /api/import/template.csv     a starter CSV with the right header
GET    /api/sources                 which sources this deployment can use
POST   /api/sources/henrik/sync     pull matches for a Riot ID
POST   /api/sources/henrik/probe    fetch without importing and report the field mapping

POST   /api/demo/seed?sessions=45   regenerate demo history
DELETE /api/demo/reset              delete all data
```

Every `/api/stats/*` endpoint accepts `days`, `map` and `mode` filters, e.g.
`/api/stats/maps?days=30&mode=Premier`.

Importing a CSV, previewing first:

```bash
curl -X POST 'http://localhost:8080/api/import/csv?dry_run=true' \
  -H 'Content-Type: text/csv' --data-binary @matches.csv
```

Logging a match:

```bash
curl -X POST http://localhost:8080/api/matches \
  -H 'Content-Type: application/json' \
  -d '{
    "played_at": "2026-09-15T21:30:00",
    "map": "Ascent",
    "mode": "Competitive",
    "rounds_won": 13,
    "rounds_lost": 9,
    "performances": [
      {"player_id": 1, "agent": "Jett", "kills": 22, "deaths": 15,
       "assists": 4, "acs": 271, "first_bloods": 6, "first_deaths": 3}
    ]
  }'
```

## Published images

Every push to `main` publishes `ghcr.io/scopeddlol/valops:latest`; version tags
(`v1.2.3`) publish `1.2.3`, `1.2` and `latest`. Every commit also gets a
`sha-<short>` tag, so you can pin to an exact build and roll back to it.

Pull requests build the image too, without publishing — that is what keeps the
Dockerfile working between releases.

To use the published image with compose, swap the `build:` line in
`docker-compose.yml` for:

```yaml
    image: ghcr.io/scopeddlol/valops:latest
```

GHCR packages start out **private**. If you want the rest of the stack to pull
without authenticating, open the package under the repository's *Packages* tab
and change its visibility to public; otherwise each of them needs a
`docker login ghcr.io` with a token that has `read:packages`.

Images are `linux/amd64`. If you self-host on ARM (a Pi, an Apple silicon
machine, a Graviton box) see the note at the top of
`.github/workflows/docker.yml`.

## Stack

- **Backend** — Rust: axum, sqlx, SQLite. One binary that serves the API and the
  built frontend, with migrations embedded at compile time.
- **Frontend** — TypeScript: React and Vite. Charts are hand-rolled SVG (no
  charting dependency); fonts are bundled, so the UI needs no internet at all.
- **Image** — multi-stage build, non-root, ~9.5 MB binary on `debian:bookworm-slim`.
