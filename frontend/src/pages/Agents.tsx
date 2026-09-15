import { useMemo, useState } from 'react';

import { api, type AgentStat, type Catalog, type PlayerStat } from '../api';
import { useAsync } from '../hooks';
import { useFilters } from '../App';
import { TopBar, WindowPicker } from '../components/Layout';
import { BarChart, ChartFrame, Heatmap } from '../components/Charts';
import { CellBar, Empty, ErrorBanner, Loading, RoleTag } from '../components/Ui';
import { num, pct, roleColor, winRateColor } from '../lib';

type SortKey = 'agent' | 'picks' | 'win_rate' | 'kd' | 'avg_acs' | 'first_blood_rate';

interface Bundle {
  agents: AgentStat[];
  players: PlayerStat[];
  catalog: Catalog;
}

const ROLES = ['All', 'Duelist', 'Initiator', 'Controller', 'Sentinel'] as const;

export default function Agents() {
  const { days, setDays, version } = useFilters();
  const [role, setRole] = useState<(typeof ROLES)[number]>('All');
  const [sort, setSort] = useState<{ key: SortKey; dir: 1 | -1 }>({ key: 'picks', dir: -1 });
  const [minPicks, setMinPicks] = useState(1);

  const { data, error, loading, reload } = useAsync<Bundle>(async () => {
    const [agents, players, catalog] = await Promise.all([
      api.agentStats({ days }),
      api.playerStats({ days }),
      api.catalog(),
    ]);
    return { agents, players, catalog };
  }, [days, version]);

  const filtered = useMemo(() => {
    const rows = (data?.agents ?? []).filter(
      (a) => (role === 'All' || a.role === role) && a.picks >= minPicks,
    );
    return [...rows].sort((a, b) => {
      if (sort.key === 'agent') return a.agent.localeCompare(b.agent) * sort.dir;
      return ((a[sort.key] as number) - (b[sort.key] as number)) * sort.dir;
    });
  }, [data, role, sort, minPicks]);

  const unplayed = useMemo(() => {
    const played = new Set((data?.agents ?? []).map((a) => a.agent));
    return (data?.catalog.agents ?? []).filter(
      (a) => !played.has(a.name) && (role === 'All' || a.role === role),
    );
  }, [data, role]);

  // Player × agent mastery, limited to agents the stack actually runs.
  const heat = useMemo(() => {
    const players = (data?.players ?? []).filter((p) => p.matches > 0);
    const agents = [...(data?.agents ?? [])]
      .filter((a) => role === 'All' || a.role === role)
      .sort((a, b) => b.picks - a.picks)
      .slice(0, 12)
      .map((a) => a.agent);
    const cells = players.map((p) =>
      agents.map((agentName) => {
        const stat = data?.agents
          .find((a) => a.agent === agentName)
          ?.players.find((ap) => ap.player_id === p.player_id);
        return stat ? { value: stat.win_rate, count: stat.picks } : null;
      }),
    );
    return { rowLabels: players.map((p) => p.name), colLabels: agents, cells };
  }, [data, role]);

  const header = (key: SortKey, label: string) => (
    <th
      className="sortable"
      onClick={() => setSort((s) => ({ key, dir: s.key === key && s.dir === -1 ? 1 : -1 }))}
    >
      {label}
      {sort.key === key && <span className="caret">{sort.dir === -1 ? '▼' : '▲'}</span>}
    </th>
  );

  return (
    <>
      <TopBar title="Agents" subtitle="Your pool, and what it is actually worth">
        <WindowPicker days={days} onChange={setDays} />
        <div className="seg">
          {ROLES.map((r) => (
            <button key={r} aria-pressed={role === r} onClick={() => setRole(r)}>
              {r}
            </button>
          ))}
        </div>
      </TopBar>

      <div className="page fade-in">
        {error && <ErrorBanner message={error} onRetry={reload} />}
        {!data && loading && <Loading rows={2} height={200} />}

        {data && data.agents.length === 0 && (
          <div className="card">
            <Empty
              title="No agent data yet"
              message="Agent stats build up from the scoreboards you log with each match."
            />
          </div>
        )}

        {filtered.length > 0 && (
          <>
            <div className="grid split-2-1">
              <ChartFrame
                title="Most played"
                subtitle="Bar length is picks. Hover for the record behind each one."
              >
                <BarChart
                  data={filtered.slice(0, 12).map((a) => ({
                    label: a.agent,
                    value: a.picks,
                    color: roleColor(a.role),
                    rows: [
                      { label: 'Win rate', value: pct(a.win_rate) },
                      { label: 'K/D', value: num(a.kd) },
                      { label: 'Role', value: a.role },
                    ],
                  }))}
                  format={(v) => String(Math.round(v))}
                  baseline={0}
                  labelWidth={88}
                  valueLabel="Picks"
                  scale="sequential"
                />
                <div className="legend" style={{ paddingTop: 14 }}>
                  {['Duelist', 'Initiator', 'Controller', 'Sentinel'].map((r) => (
                    <span className="legend-item" key={r}>
                      <span className="legend-swatch" style={{ background: roleColor(r) }} />
                      {r}
                    </span>
                  ))}
                </div>
              </ChartFrame>

              <ChartFrame
                title="Win rate"
                subtitle="Agents with 5+ picks, diverging from an even 50%"
              >
                <BarChart
                  data={filtered
                    .filter((a) => a.picks >= 5)
                    .slice()
                    .sort((a, b) => b.win_rate - a.win_rate)
                    .slice(0, 10)
                    .map((a) => ({
                      label: a.agent,
                      value: a.win_rate,
                      rows: [{ label: 'Picks', value: String(a.picks) }],
                    }))}
                  format={(v) => pct(v)}
                  baseline={0.5}
                  valueLabel="Win rate"
                  labelWidth={82}
                />
              </ChartFrame>
            </div>

            {heat.colLabels.length > 0 && (
              <ChartFrame
                title="Who plays what"
                subtitle="Win rate per player per agent. Faded cells have few games behind them."
              >
                <Heatmap
                  rowLabels={heat.rowLabels}
                  colLabels={heat.colLabels}
                  cells={heat.cells}
                  format={(v) => pct(v)}
                />
              </ChartFrame>
            )}

            <section className="card">
              <div className="card-head">
                <div className="grow">
                  <div className="card-title">All agents played</div>
                  <div className="card-sub">{filtered.length} agents in this view</div>
                </div>
                <div className="seg">
                  {[1, 3, 10].map((n) => (
                    <button key={n} aria-pressed={minPicks === n} onClick={() => setMinPicks(n)}>
                      {n}+ picks
                    </button>
                  ))}
                </div>
              </div>
              <div className="card-body flush">
                <div className="table-wrap">
                  <table className="data">
                    <thead>
                      <tr>
                        {header('agent', 'Agent')}
                        <th>Role</th>
                        {header('picks', 'Picks')}
                        {header('win_rate', 'Win rate')}
                        {header('kd', 'K/D')}
                        {header('avg_acs', 'ACS')}
                        {header('first_blood_rate', 'Opening duels')}
                        <th>Main</th>
                      </tr>
                    </thead>
                    <tbody>
                      {filtered.map((a) => (
                        <tr key={a.agent}>
                          <td style={{ fontWeight: 600 }}>{a.agent}</td>
                          <td style={{ textAlign: 'right' }}>
                            <RoleTag role={a.role} />
                          </td>
                          <td className="dim">{a.picks}</td>
                          <td>
                            <CellBar value={a.win_rate} color={winRateColor(a.win_rate)} />{' '}
                            <span className="num">{pct(a.win_rate)}</span>
                          </td>
                          <td>{num(a.kd)}</td>
                          <td className="dim">{Math.round(a.avg_acs)}</td>
                          <td className="dim">{pct(a.first_blood_rate)}</td>
                          <td className="dim">
                            {a.players[0] ? `${a.players[0].player} (${a.players[0].picks})` : '—'}
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              </div>
            </section>

            {unplayed.length > 0 && (
              <section className="card">
                <div className="card-head">
                  <div className="grow">
                    <div className="card-title">Never touched</div>
                    <div className="card-sub">
                      {unplayed.length} agent{unplayed.length === 1 ? '' : 's'} with no games logged — blind spots in the pool
                    </div>
                  </div>
                </div>
                <div className="card-body">
                  <div className="row wrap" style={{ gap: 7 }}>
                    {unplayed.map((a) => (
                      <span className="tag" key={a.name}>
                        <span className={`role-dot role-${a.role}`} style={{ marginRight: 6 }} />
                        {a.name}
                      </span>
                    ))}
                  </div>
                </div>
              </section>
            )}
          </>
        )}
      </div>
    </>
  );
}
