import { useNavigate } from 'react-router-dom';
import { useState } from 'react';

import { api, type MapStat, type Overview, type PlayerStat } from '../api';
import { useAsync } from '../hooks';
import { useFilters } from '../App';
import { TopBar, WindowPicker } from '../components/Layout';
import { BarChart, ChartFrame, FormStrip, LineChart, Sparkline } from '../components/Charts';
import { Avatar, Empty, ErrorBanner, Loading, Meter, ResultPill, StatTile } from '../components/Ui';
import { num, pct, relativeDate, seriesColor, shortDate, signed } from '../lib';

interface Bundle {
  overview: Overview;
  maps: MapStat[];
  players: PlayerStat[];
}

export default function Dashboard() {
  const { days, setDays, version, invalidate } = useFilters();
  const navigate = useNavigate();
  const [seeding, setSeeding] = useState(false);

  const { data, error, loading, reload } = useAsync<Bundle>(async () => {
    const filter = { days };
    const [overview, maps, players] = await Promise.all([
      api.overview(filter),
      api.mapStats(filter),
      api.playerStats(filter),
    ]);
    return { overview, maps, players };
  }, [days, version]);

  const loadDemo = async () => {
    setSeeding(true);
    try {
      await api.seedDemo();
      invalidate();
    } finally {
      setSeeding(false);
    }
  };

  const o = data?.overview;

  return (
    <>
      <TopBar
        title="Dashboard"
        subtitle={o ? `${o.matches} matches · ${o.maps_played} maps · ${o.roster_size} on the roster` : 'Loading…'}
      >
        <WindowPicker days={days} onChange={setDays} />
        <button className="btn primary" onClick={() => navigate('/builder')}>
          Build a comp
        </button>
      </TopBar>

      <div className="page fade-in">
        {error && <ErrorBanner message={error} onRetry={reload} />}
        {!data && loading && <Loading rows={3} height={120} />}

        {o && o.matches === 0 && (
          <div className="card">
            <Empty
              title="No matches logged yet"
              message="Log a match from the Matches tab, or drop in a generated six-month history to see what the app does with real volume."
              action={
                <div className="row" style={{ marginTop: 6 }}>
                  <button className="btn primary" onClick={loadDemo} disabled={seeding}>
                    {seeding ? 'Generating…' : 'Load demo history'}
                  </button>
                  <button className="btn ghost" onClick={() => navigate('/matches')}>
                    Log a match
                  </button>
                </div>
              }
            />
          </div>
        )}

        {data && o && o.matches > 0 && (
          <>
            {/* Hero -------------------------------------------------- */}
            <section className="hero">
              <div>
                <div className="stat-label">Record</div>
                <div className="hero-record">
                  <span style={{ color: 'var(--good-ink)' }}>{o.wins}</span>
                  <span className="sep">–</span>
                  <span style={{ color: 'var(--critical-ink)' }}>{o.losses}</span>
                </div>
                <div className="stat-foot">
                  {pct(o.win_rate, 1)} win rate
                  {o.streak !== 0 && (
                    <span className={o.streak > 0 ? 'delta-up' : 'delta-down'}>
                      · {Math.abs(o.streak)} {o.streak > 0 ? 'W' : 'L'} streak
                    </span>
                  )}
                </div>
              </div>

              <div style={{ minWidth: 200 }}>
                <div className="stat-label" style={{ marginBottom: 8 }}>
                  Recent form
                </div>
                <FormStrip
                  entries={[...o.form].reverse().map((f) => ({
                    result: f.result,
                    title: `${f.map} ${f.rounds_won}–${f.rounds_lost} · ${shortDate(f.played_at)}`,
                  }))}
                />
                <div className="stat-foot">Oldest to newest, last {o.form.length}</div>
              </div>

              <div className="spacer" />

              {o.best_map && (
                <div style={{ minWidth: 148 }}>
                  <div className="stat-label">Best map</div>
                  <div className="hero-record" style={{ fontSize: 26 }}>
                    {o.best_map.map}
                  </div>
                  <div className="stat-foot">
                    {o.best_map.wins}–{o.best_map.losses} · {pct(o.best_map.win_rate)}
                  </div>
                </div>
              )}
              {o.worst_map && (
                <div style={{ minWidth: 148 }}>
                  <div className="stat-label">Problem map</div>
                  <div className="hero-record" style={{ fontSize: 26 }}>
                    {o.worst_map.map}
                  </div>
                  <div className="stat-foot">
                    {o.worst_map.wins}–{o.worst_map.losses} · {pct(o.worst_map.win_rate)}
                  </div>
                </div>
              )}
            </section>

            {/* Tiles ------------------------------------------------- */}
            <div className="grid cols-4">
              <StatTile
                label="Win rate"
                value={pct(o.win_rate, 1)}
                accent="var(--s1)"
                foot={`${o.wins} of ${o.matches} matches`}
              />
              <StatTile
                label="Team K/D"
                value={num(o.team_kd)}
                accent="var(--s3)"
                foot={`${o.rounds_won + o.rounds_lost} rounds played`}
              />
              <StatTile
                label="Avg ACS"
                value={Math.round(o.avg_acs)}
                accent="var(--s4)"
                foot="Per player, per match"
              />
              <StatTile
                label="Round win rate"
                value={pct(o.round_win_rate, 1)}
                accent="var(--s5)"
                foot={`${o.rounds_won}–${o.rounds_lost} rounds`}
              />
            </div>

            {/* Map form + squad -------------------------------------- */}
            <div className="grid split-2-1">
              <ChartFrame
                title="Win rate by map"
                subtitle="Distance from an even 50%. Blue is a map you should be picking; red is one to ban."
              >
                <BarChart
                  data={data.maps
                    .filter((m) => m.played > 0)
                    .map((m) => ({
                      label: m.map,
                      value: m.win_rate,
                      rows: [
                        { label: 'Record', value: `${m.wins}–${m.losses}` },
                        { label: 'Rounds', value: `${m.rounds_won}–${m.rounds_lost}` },
                        { label: 'Team K/D', value: num(m.team_kd) },
                        { label: 'Last played', value: relativeDate(m.last_played) },
                      ],
                    }))}
                  format={(v) => pct(v)}
                  baseline={0.5}
                  domain={[0, 1]}
                  valueLabel="Win rate"
                  onSelect={() => navigate('/maps')}
                />
              </ChartFrame>

              <ChartFrame title="Squad" subtitle="Ordered by composite rating">
                <div className="grid" style={{ gap: 16 }}>
                  {data.players
                    .filter((p) => p.matches > 0)
                    .map((p, i) => (
                      <div key={p.player_id} className="row" style={{ gap: 12, alignItems: 'flex-start' }}>
                        <Avatar name={p.name} index={i} size="sm" />
                        <div style={{ flex: 1, minWidth: 0 }}>
                          <div className="row between">
                            <strong style={{ fontSize: 13.5 }}>{p.name}</strong>
                            <span className="num" style={{ fontSize: 12, color: 'var(--ink-2)' }}>
                              {num(p.kd)} K/D
                            </span>
                          </div>
                          <div className="row between" style={{ fontSize: 11.5, color: 'var(--ink-3)', marginTop: 2 }}>
                            <span>
                              {p.best_agents[0] ? p.best_agents[0].agent : p.role}
                              {' · '}
                              {Math.round(p.avg_acs)} ACS
                            </span>
                            <span className="num">{signed(p.entry_diff)} entries</span>
                          </div>
                          <div style={{ marginTop: 6 }}>
                            <Meter value={p.rating} color={seriesColor(i)} />
                          </div>
                        </div>
                      </div>
                    ))}
                </div>
              </ChartFrame>
            </div>

            {/* Trend -------------------------------------------------- */}
            <ChartFrame
              title="Rolling win rate"
              subtitle="Trailing ten matches. The dashed line is an even 50%."
            >
              <LineChart
                series={[
                  {
                    name: 'Win rate (last 10)',
                    color: '#3987e5',
                    points: o.timeline.map((t) => ({
                      x: t.played_at,
                      y: t.rolling_win_rate,
                      label: `${shortDate(t.played_at)} · ${t.map}`,
                    })),
                  },
                ]}
                yFormat={(v) => pct(v)}
                yDomain={[0, 1]}
                reference={0.5}
                area
                height={230}
              />
            </ChartFrame>

            {/* Highlights -------------------------------------------- */}
            <div className="grid cols-3">
              {o.mvp && (
                <div className="card">
                  <div className="card-head">
                    <div className="grow">
                      <div className="card-title">Squad MVP</div>
                      <div className="card-sub">Impact, trading and results combined</div>
                    </div>
                  </div>
                  <div className="card-body">
                    <div className="row" style={{ gap: 14 }}>
                      <Avatar name={o.mvp.name} index={data.players.findIndex((p) => p.player_id === o.mvp!.player_id)} />
                      <div>
                        <div style={{ fontFamily: 'var(--font-display)', fontSize: 20, fontWeight: 700 }}>
                          {o.mvp.name}
                        </div>
                        <div className="card-sub">
                          {num(o.mvp.kd)} K/D · {Math.round(o.mvp.avg_acs)} ACS · {signed(o.mvp.entry_diff)} entries
                        </div>
                      </div>
                    </div>
                    <div style={{ marginTop: 14 }}>
                      <Sparkline
                        values={o.mvp.timeline.slice(-24).map((t) => t.kd)}
                        color={seriesColor(data.players.findIndex((p) => p.player_id === o.mvp!.player_id))}
                        reference={1}
                      />
                      <div className="card-sub">K/D across their last {Math.min(24, o.mvp.timeline.length)} matches</div>
                    </div>
                  </div>
                </div>
              )}

              {o.top_agent && (
                <div className="card">
                  <div className="card-head">
                    <div className="grow">
                      <div className="card-title">Best agent</div>
                      <div className="card-sub">Best record you can trust, 5+ picks</div>
                    </div>
                  </div>
                  <div className="card-body">
                    <div style={{ fontFamily: 'var(--font-display)', fontSize: 26, fontWeight: 700 }}>
                      {o.top_agent.agent}
                    </div>
                    <div className="card-sub" style={{ marginBottom: 12 }}>
                      {o.top_agent.role} · {o.top_agent.picks} picks · {pct(o.top_agent.win_rate)} win rate
                    </div>
                    <div className="row wrap" style={{ gap: 6 }}>
                      {o.top_agent.players.slice(0, 3).map((p) => (
                        <span className="tag" key={p.player_id}>
                          {p.player} · {p.picks}× · {pct(p.win_rate)}
                        </span>
                      ))}
                    </div>
                    <div className="card-sub" style={{ marginTop: 14, marginBottom: 6 }}>
                      Played most on
                    </div>
                    <div className="row wrap" style={{ gap: 6 }}>
                      {o.top_agent.maps.slice(0, 4).map((m) => (
                        <span className="tag" key={m.map}>
                          {m.map} · {m.picks}× · {pct(m.win_rate)}
                        </span>
                      ))}
                    </div>
                  </div>
                </div>
              )}

              <div className="card">
                <div className="card-head">
                  <div className="grow">
                    <div className="card-title">Last five</div>
                    <div className="card-sub">Most recent first</div>
                  </div>
                </div>
                <div className="card-body" style={{ display: 'grid', gap: 9 }}>
                  {o.form.slice(0, 5).map((f) => (
                    <div className="row between" key={f.match_id}>
                      <div className="row" style={{ gap: 9 }}>
                        <ResultPill result={f.result} />
                        <span style={{ fontSize: 13 }}>{f.map}</span>
                      </div>
                      <div className="row" style={{ gap: 10 }}>
                        <span className="num" style={{ fontSize: 13, color: 'var(--ink-2)' }}>
                          {f.rounds_won}–{f.rounds_lost}
                        </span>
                        <span style={{ fontSize: 12, color: 'var(--ink-3)', minWidth: 52, textAlign: 'right' }}>
                          {shortDate(f.played_at)}
                        </span>
                      </div>
                    </div>
                  ))}
                </div>
              </div>
            </div>
          </>
        )}
      </div>
    </>
  );
}
