import { useMemo, useState } from 'react';
import { useNavigate } from 'react-router-dom';

import { api, type MapStat, type PlayerStat } from '../api';
import { useAsync } from '../hooks';
import { useFilters } from '../App';
import { TopBar, WindowPicker } from '../components/Layout';
import { BarChart, ChartFrame, Heatmap } from '../components/Charts';
import { CellBar, Empty, ErrorBanner, Loading, ResultPill } from '../components/Ui';
import { num, pct, relativeDate, signed, winRateColor } from '../lib';

type SortKey = 'map' | 'played' | 'win_rate' | 'round_win_rate' | 'avg_round_diff' | 'team_kd' | 'avg_acs';

interface Bundle {
  maps: MapStat[];
  players: PlayerStat[];
}

export default function Maps() {
  const { days, setDays, version } = useFilters();
  const navigate = useNavigate();
  const [sort, setSort] = useState<{ key: SortKey; dir: 1 | -1 }>({ key: 'win_rate', dir: -1 });
  const [focus, setFocus] = useState<string | null>(null);
  const [onlyActive, setOnlyActive] = useState(false);

  const { data, error, loading, reload } = useAsync<Bundle>(async () => {
    const [maps, players] = await Promise.all([api.mapStats({ days }), api.playerStats({ days })]);
    return { maps, players };
  }, [days, version]);

  const visible = useMemo(() => {
    const rows = (data?.maps ?? []).filter((m) => m.played > 0 && (!onlyActive || m.active));
    const sorted = [...rows].sort((a, b) => {
      if (sort.key === 'map') return a.map.localeCompare(b.map) * sort.dir;
      return ((a[sort.key] as number) - (b[sort.key] as number)) * sort.dir;
    });
    return sorted;
  }, [data, sort, onlyActive]);

  const focused = visible.find((m) => m.map === focus) ?? null;

  /**
   * Player × map K/D.
   *
   * Win rate is deliberately *not* the measure here: a five-stack plays every
   * match together, so per-player win rate per map is identical for everyone
   * and the grid says nothing. K/D is individual, so this actually answers
   * "who carries their weight where".
   */
  const heat = useMemo(() => {
    const players = (data?.players ?? []).filter((p) => p.matches > 0);
    const maps = visible.slice(0, 10).map((m) => m.map);
    const cells = players.map((p) =>
      maps.map((m) => {
        const row = p.best_maps.find((bm) => bm.map === m);
        return row ? { value: row.kd, count: row.played } : null;
      }),
    );
    return { rowLabels: players.map((p) => p.name), colLabels: maps, cells };
  }, [data, visible]);

  // K/D is neutral at 1.0, not 0.5, so re-centre it for the diverging scale.
  const kdToScale = (kd: number) => Math.max(0, Math.min(1, 0.5 + (kd - 1) * 0.5));

  const header = (key: SortKey, label: string) => (
    <th
      className="sortable"
      onClick={() => setSort((s) => ({ key, dir: s.key === key && s.dir === -1 ? 1 : -1 }))}
      title={`Sort by ${label}`}
    >
      {label}
      {sort.key === key && <span className="caret">{sort.dir === -1 ? '▼' : '▲'}</span>}
    </th>
  );

  return (
    <>
      <TopBar title="Maps" subtitle="Where to pick, where to ban">
        <WindowPicker days={days} onChange={setDays} />
        <div className="seg">
          <button aria-pressed={!onlyActive} onClick={() => setOnlyActive(false)}>
            All maps
          </button>
          <button aria-pressed={onlyActive} onClick={() => setOnlyActive(true)}>
            In rotation
          </button>
        </div>
      </TopBar>

      <div className="page fade-in">
        {error && <ErrorBanner message={error} onRetry={reload} />}
        {!data && loading && <Loading rows={2} height={200} />}

        {data && visible.length === 0 && (
          <div className="card">
            <Empty
              title="No map data yet"
              message="Once matches are logged, every map you play shows up here with its own record, round differential and agent picks."
              action={
                <button className="btn primary" onClick={() => navigate('/matches')}>
                  Log a match
                </button>
              }
            />
          </div>
        )}

        {visible.length > 0 && (
          <>
            <ChartFrame
              title="Win rate by map"
              subtitle="Diverging from an even 50%. Bar length is the edge; hover for the full record."
            >
              <BarChart
                data={visible.map((m) => ({
                  label: m.map,
                  value: m.win_rate,
                  rows: [
                    { label: 'Record', value: `${m.wins}–${m.losses}` },
                    { label: 'Round diff', value: signed(m.avg_round_diff, 1) },
                    { label: 'Team K/D', value: num(m.team_kd) },
                  ],
                }))}
                format={(v) => pct(v)}
                baseline={0.5}
                domain={[0, 1]}
                valueLabel="Win rate"
                onSelect={(label) => setFocus(label === focus ? null : label)}
              />
            </ChartFrame>

            <section className="card">
              <div className="card-head">
                <div className="grow">
                  <div className="card-title">Map table</div>
                  <div className="card-sub">Click a row to break the map down</div>
                </div>
              </div>
              <div className="card-body flush">
                <div className="table-wrap">
                  <table className="data">
                    <thead>
                      <tr>
                        {header('map', 'Map')}
                        {header('played', 'Played')}
                        <th>Record</th>
                        {header('win_rate', 'Win rate')}
                        {header('round_win_rate', 'Round win %')}
                        {header('avg_round_diff', 'Avg diff')}
                        {header('team_kd', 'Team K/D')}
                        {header('avg_acs', 'ACS')}
                        <th>Last played</th>
                      </tr>
                    </thead>
                    <tbody>
                      {visible.map((m) => (
                        <tr
                          key={m.map}
                          className="clickable"
                          onClick={() => setFocus(m.map === focus ? null : m.map)}
                          style={m.map === focus ? { background: 'rgba(57,135,229,0.08)' } : undefined}
                        >
                          <td>
                            <span className="row" style={{ gap: 8 }}>
                              {m.map}
                              {!m.active && <span className="tag">out of pool</span>}
                            </span>
                          </td>
                          <td className="dim">{m.played}</td>
                          <td>
                            {m.wins}–{m.losses}
                          </td>
                          <td>
                            <CellBar value={m.win_rate} color={winRateColor(m.win_rate)} />{' '}
                            <span className="num">{pct(m.win_rate)}</span>
                          </td>
                          <td>{pct(m.round_win_rate, 1)}</td>
                          <td style={{ color: m.avg_round_diff >= 0 ? 'var(--good-ink)' : 'var(--critical-ink)' }}>
                            {signed(m.avg_round_diff, 1)}
                          </td>
                          <td>{num(m.team_kd)}</td>
                          <td className="dim">{Math.round(m.avg_acs)}</td>
                          <td className="dim">{relativeDate(m.last_played)}</td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              </div>
            </section>

            {focused && (
              <div className="grid split-2-1">
                <ChartFrame
                  title={`${focused.map} · agent picks`}
                  subtitle="Most played agents on this map and how those games went"
                  actions={
                    <button className="btn sm ghost" onClick={() => setFocus(null)}>
                      Close
                    </button>
                  }
                >
                  <BarChart
                    data={focused.top_agents.map((a) => ({
                      label: a.agent,
                      value: a.win_rate,
                      rows: [
                        { label: 'Picks', value: String(a.picks) },
                        { label: 'Role', value: a.role },
                      ],
                    }))}
                    format={(v) => pct(v)}
                    baseline={0.5}
                    domain={[0, 1]}
                    valueLabel="Win rate"
                    labelWidth={86}
                  />
                </ChartFrame>

                <ChartFrame title="At a glance" subtitle={`${focused.played} matches`}>
                  <div className="grid" style={{ gap: 12 }}>
                    <div className="row between">
                      <span style={{ color: 'var(--ink-2)' }}>Record</span>
                      <ResultPill
                        result={focused.wins >= focused.losses ? 'win' : 'loss'}
                        score={`${focused.wins}–${focused.losses}`}
                      />
                    </div>
                    <div className="row between">
                      <span style={{ color: 'var(--ink-2)' }}>Rounds</span>
                      <span className="num">
                        {focused.rounds_won}–{focused.rounds_lost} ({pct(focused.round_win_rate)})
                      </span>
                    </div>
                    <div className="row between">
                      <span style={{ color: 'var(--ink-2)' }}>Avg round diff</span>
                      <span className="num">{signed(focused.avg_round_diff, 1)}</span>
                    </div>
                    <div className="row between">
                      <span style={{ color: 'var(--ink-2)' }}>Team K/D</span>
                      <span className="num">{num(focused.team_kd)}</span>
                    </div>
                    <div className="row between">
                      <span style={{ color: 'var(--ink-2)' }}>Smoothed rating</span>
                      <span className="num">{pct(focused.rating)}</span>
                    </div>
                    <button className="btn primary" onClick={() => navigate('/builder')} style={{ marginTop: 6 }}>
                      Build a comp for {focused.map}
                    </button>
                  </div>
                </ChartFrame>
              </div>
            )}

            {heat.rowLabels.length > 0 && heat.colLabels.length > 0 && (
              <ChartFrame
                title="Who carries where"
                subtitle="K/D per player per map. You all win and lose together, so K/D — not win rate — is what separates you."
              >
                <Heatmap
                  rowLabels={heat.rowLabels}
                  colLabels={heat.colLabels}
                  cells={heat.cells}
                  format={(v) => v.toFixed(2)}
                  normalize={kdToScale}
                />
                <div className="legend" style={{ paddingTop: 14 }}>
                  <span className="legend-item">
                    <span className="legend-swatch" style={{ background: winRateColor(kdToScale(1.4)) }} /> 1.40+ K/D
                  </span>
                  <span className="legend-item">
                    <span className="legend-swatch" style={{ background: winRateColor(kdToScale(1)) }} /> even at 1.00
                  </span>
                  <span className="legend-item">
                    <span className="legend-swatch" style={{ background: winRateColor(kdToScale(0.6)) }} /> 0.60 and under
                  </span>
                  <span className="legend-item">
                    <span className="legend-swatch" style={{ background: 'rgba(255,255,255,0.05)' }} /> never played
                  </span>
                </div>
              </ChartFrame>
            )}

          </>
        )}
      </div>
    </>
  );
}
