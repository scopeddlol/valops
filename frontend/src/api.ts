/** Typed client for the valops API. Types mirror the Rust structs 1:1. */

export type Role = 'Duelist' | 'Initiator' | 'Controller' | 'Sentinel' | 'Flex';
export type Result = 'win' | 'loss' | 'draw';

export interface Agent { name: string; role: Role }
export interface MapInfo { name: string; active: boolean }
export interface Catalog { agents: Agent[]; maps: MapInfo[]; roles: Role[] }

export interface Player {
  id: number;
  name: string;
  riot_id: string | null;
  role: string;
  rank: string | null;
  active: boolean;
  created_at: string;
}

export interface PlayerInput {
  name: string;
  riot_id?: string | null;
  role: string;
  rank?: string | null;
  active: boolean;
}

export interface Performance {
  id: number;
  match_id: number;
  player_id: number;
  agent: string;
  kills: number;
  deaths: number;
  assists: number;
  acs: number;
  first_bloods: number;
  first_deaths: number;
  plants: number;
  defuses: number;
  player_name: string;
  agent_role: Role;
}

export interface MatchDetail {
  id: number;
  played_at: string;
  map: string;
  mode: string;
  rounds_won: number;
  rounds_lost: number;
  notes: string | null;
  result: Result;
  performances: Performance[];
}

export interface PerformanceInput {
  player_id: number;
  agent: string;
  kills: number;
  deaths: number;
  assists: number;
  acs: number;
  first_bloods: number;
  first_deaths: number;
  plants: number;
  defuses: number;
}

export interface MatchInput {
  played_at: string;
  map: string;
  mode: string;
  rounds_won: number;
  rounds_lost: number;
  notes?: string | null;
  performances: PerformanceInput[];
}

export interface AgentPick {
  agent: string;
  role: Role;
  picks: number;
  win_rate: number;
  player: string | null;
}

export interface MapStat {
  map: string;
  active: boolean;
  played: number;
  wins: number;
  losses: number;
  draws: number;
  win_rate: number;
  rating: number;
  rounds_won: number;
  rounds_lost: number;
  round_win_rate: number;
  avg_round_diff: number;
  team_kd: number;
  avg_acs: number;
  last_played: string | null;
  top_agents: AgentPick[];
}

export interface AgentPlayerStat {
  agent: string;
  player_id: number;
  player: string;
  picks: number;
  win_rate: number;
  rating: number;
  kd: number;
  avg_acs: number;
}

export interface AgentMapStat { map: string; picks: number; win_rate: number }

export interface AgentStat {
  agent: string;
  role: Role;
  picks: number;
  wins: number;
  win_rate: number;
  rating: number;
  kd: number;
  avg_acs: number;
  avg_kills: number;
  first_blood_rate: number;
  players: AgentPlayerStat[];
  maps: AgentMapStat[];
}

export interface PlayerMapStat {
  map: string;
  played: number;
  win_rate: number;
  rating: number;
  kd: number;
  avg_acs: number;
}

export interface PlayerTimelinePoint {
  match_id: number;
  played_at: string;
  map: string;
  agent: string;
  result: Result;
  kd: number;
  acs: number;
}

export interface PlayerStat {
  player_id: number;
  name: string;
  riot_id: string | null;
  role: string;
  rank: string | null;
  active: boolean;
  matches: number;
  wins: number;
  win_rate: number;
  kills: number;
  deaths: number;
  assists: number;
  kd: number;
  kda: number;
  avg_kills: number;
  avg_deaths: number;
  avg_acs: number;
  first_bloods: number;
  first_deaths: number;
  entry_diff: number;
  plants: number;
  defuses: number;
  agent_pool: number;
  rating: number;
  best_agents: AgentPlayerStat[];
  best_maps: PlayerMapStat[];
  timeline: PlayerTimelinePoint[];
}

export interface FormEntry {
  match_id: number;
  played_at: string;
  map: string;
  result: Result;
  rounds_won: number;
  rounds_lost: number;
}

export interface TimelinePoint {
  match_id: number;
  played_at: string;
  map: string;
  result: Result;
  round_diff: number;
  team_kd: number;
  avg_acs: number;
  rolling_win_rate: number;
}

export interface Overview {
  matches: number;
  wins: number;
  losses: number;
  draws: number;
  win_rate: number;
  rounds_won: number;
  rounds_lost: number;
  round_win_rate: number;
  team_kd: number;
  avg_acs: number;
  first_blood_rate: number;
  streak: number;
  maps_played: number;
  agents_played: number;
  roster_size: number;
  form: FormEntry[];
  timeline: TimelinePoint[];
  best_map: MapStat | null;
  worst_map: MapStat | null;
  top_agent: AgentStat | null;
  mvp: PlayerStat | null;
}

export interface CompStat {
  agents: string[];
  roles: Role[];
  played: number;
  wins: number;
  win_rate: number;
  rating: number;
  maps: string[];
  last_played: string | null;
}

export interface AgentOption {
  agent: string;
  role: Role;
  score: number;
  win_rate_on_map: number | null;
  win_rate_overall: number | null;
  picks_on_map: number;
  picks_overall: number;
  kd: number | null;
  avg_acs: number | null;
  confidence: number;
}

export interface BuilderSlot extends AgentOption {
  player_id: number;
  player: string;
  preferred_role: string;
  locked: boolean;
  reasons: string[];
  alternatives: AgentOption[];
}

export interface BuilderResult {
  map: string;
  slots: BuilderSlot[];
  role_spread: Record<string, number>;
  comp_score: number;
  confidence: number;
  notes: string[];
  history: CompStat | null;
  map_record: string | null;
}

export interface StatsFilter {
  days?: number | null;
  map?: string | null;
  mode?: string | null;
}

// ---------------------------------------------------------------------------

export class ApiError extends Error {
  constructor(message: string, readonly status: number) {
    super(message);
    this.name = 'ApiError';
  }
}

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const res = await fetch(`/api${path}`, {
    headers: init?.body ? { 'Content-Type': 'application/json' } : undefined,
    ...init,
  });
  if (!res.ok) {
    let message = `Request failed (${res.status})`;
    try {
      const body = await res.json();
      if (body?.error) message = body.error;
    } catch {
      /* the body was not JSON; the status-derived message stands */
    }
    throw new ApiError(message, res.status);
  }
  if (res.status === 204) return undefined as T;
  return res.json() as Promise<T>;
}

function qs(filter: StatsFilter | undefined): string {
  if (!filter) return '';
  const params = new URLSearchParams();
  if (filter.days) params.set('days', String(filter.days));
  if (filter.map) params.set('map', filter.map);
  if (filter.mode) params.set('mode', filter.mode);
  const s = params.toString();
  return s ? `?${s}` : '';
}

export const api = {
  catalog: () => request<Catalog>('/catalog'),

  players: () => request<Player[]>('/players'),
  createPlayer: (input: PlayerInput) =>
    request<Player>('/players', { method: 'POST', body: JSON.stringify(input) }),
  updatePlayer: (id: number, input: PlayerInput) =>
    request<Player>(`/players/${id}`, { method: 'PATCH', body: JSON.stringify(input) }),
  deletePlayer: (id: number) => request<unknown>(`/players/${id}`, { method: 'DELETE' }),

  matches: (limit = 200) => request<MatchDetail[]>(`/matches?limit=${limit}`),
  createMatch: (input: MatchInput) =>
    request<MatchDetail>('/matches', { method: 'POST', body: JSON.stringify(input) }),
  deleteMatch: (id: number) => request<unknown>(`/matches/${id}`, { method: 'DELETE' }),

  overview: (f?: StatsFilter) => request<Overview>(`/stats/overview${qs(f)}`),
  mapStats: (f?: StatsFilter) => request<MapStat[]>(`/stats/maps${qs(f)}`),
  agentStats: (f?: StatsFilter) => request<AgentStat[]>(`/stats/agents${qs(f)}`),
  playerStats: (f?: StatsFilter) => request<PlayerStat[]>(`/stats/players${qs(f)}`),
  compStats: (f?: StatsFilter) => request<CompStat[]>(`/stats/comps${qs(f)}`),

  builder: (map: string, players?: number[], locks?: Record<number, string>, exclude?: string[]) => {
    const params = new URLSearchParams({ map });
    if (players?.length) params.set('players', players.join(','));
    const lockPairs = Object.entries(locks ?? {}).filter(([, agent]) => !!agent);
    if (lockPairs.length) params.set('locks', lockPairs.map(([id, a]) => `${id}:${a}`).join(','));
    if (exclude?.length) params.set('exclude', exclude.join(','));
    return request<BuilderResult>(`/builder?${params}`);
  },

  seedDemo: (sessions?: number) =>
    request<{ players: number; matches: number }>(
      `/demo/seed${sessions ? `?sessions=${sessions}` : ''}`,
      { method: 'POST' },
    ),
  resetAll: () => request<unknown>('/demo/reset', { method: 'DELETE' }),
};
