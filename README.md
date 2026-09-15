# valops

A self-hosted stats desk for a Valorant five-stack. Log your matches, see which
maps you actually win on, and get a comp built from your own history rather than
somebody else's tier list.

One container, one SQLite file. Nothing leaves your machine, and no Riot API key
is needed.

```bash
docker compose up -d --build   # then open http://localhost:8080
```

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
| `RUST_LOG` | `valops=info,tower_http=warn` | Log filter |

Change the published port with `VALOPS_PORT=9000 docker compose up -d`.

## Your data

Matches live in the `valops-data` volume, mounted at `/app/data`.

```bash
# Back up
docker compose exec valops cat /app/data/valops.db > valops-backup.db

# Or export everything as JSON (also available from the Matches page)
curl http://localhost:8080/api/export > valops-export.json
```

## Running without Docker

Two terminals; the Vite dev server proxies `/api` to the Rust process.

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

POST   /api/demo/seed?sessions=45   regenerate demo history
DELETE /api/demo/reset              delete all data
```

Every `/api/stats/*` endpoint accepts `days`, `map` and `mode` filters, e.g.
`/api/stats/maps?days=30&mode=Premier`.

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

## Stack

- **Backend** — Rust: axum, sqlx, SQLite. One binary that serves the API and the
  built frontend, with migrations embedded at compile time.
- **Frontend** — TypeScript: React and Vite. Charts are hand-rolled SVG (no
  charting dependency); fonts are bundled, so the UI needs no internet at all.
- **Image** — multi-stage build, non-root, ~5.6 MB binary on `debian:bookworm-slim`.
