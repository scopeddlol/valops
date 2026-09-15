import { useMemo, useState } from 'react';

import { api, type BuilderResult, type Catalog, type MapStat, type PlayerStat } from '../api';
import { useAsync } from '../hooks';
import { useFilters } from '../App';
import { TopBar } from '../components/Layout';
import { ChartFrame, RoleSpread } from '../components/Charts';
import { Avatar, Empty, ErrorBanner, Loading, Meter, RoleTag, StatTile } from '../components/Ui';
import { num, pct, roleColor } from '../lib';

interface Setup {
  catalog: Catalog;
  players: PlayerStat[];
  maps: MapStat[];
}

export default function Builder() {
  const { version } = useFilters();
  const [map, setMap] = useState<string>('');
  const [chosen, setChosen] = useState<number[] | null>(null);
  const [locks, setLocks] = useState<Record<number, string>>({});

  const setup = useAsync<Setup>(async () => {
    const [catalog, players, maps] = await Promise.all([
      api.catalog(),
      api.playerStats(),
      api.mapStats(),
    ]);
    return { catalog, players, maps };
  }, [version]);

  const roster = useMemo(
    () => setup.data?.players.filter((p) => p.active) ?? [],
    [setup.data],
  );

  const selected = chosen ?? roster.map((p) => p.player_id);
  // Default to the map you play most — that is where the data is richest and
  // where a recommendation is most likely to be the one you need.
  const defaultMap =
    [...(setup.data?.maps ?? [])].sort((a, b) => b.played - a.played)[0]?.map ??
    setup.data?.catalog.maps.find((m) => m.active)?.name ??
    '';
  const activeMap = map || defaultMap;

  const build = useAsync<BuilderResult | null>(async () => {
    if (!activeMap || selected.length === 0) return null;
    return api.builder(activeMap, selected, locks);
  }, [activeMap, selected.join(','), JSON.stringify(locks), version]);

  const togglePlayer = (id: number) => {
    const next = selected.includes(id) ? selected.filter((p) => p !== id) : [...selected, id];
    setChosen(next);
    // A dropped player should not keep a lock hanging around.
    if (!next.includes(id)) {
      setLocks((prev) => {
        const copy = { ...prev };
        delete copy[id];
        return copy;
      });
    }
  };

  const result = build.data;

  return (
    <>
      <TopBar
        title="Team Builder"
        subtitle="Five agents picked from what this stack actually wins with"
      >
        {Object.keys(locks).length > 0 && (
          <button className="btn ghost" onClick={() => setLocks({})}>
            Clear {Object.keys(locks).length} lock{Object.keys(locks).length === 1 ? '' : 's'}
          </button>
        )}
      </TopBar>

      <div className="page fade-in">
        {setup.error && <ErrorBanner message={setup.error} onRetry={setup.reload} />}
        {build.error && <ErrorBanner message={build.error} onRetry={build.reload} />}
        {!setup.data && setup.loading && <Loading rows={2} height={120} />}

        {setup.data && roster.length === 0 && (
          <div className="card">
            <Empty
              title="No active players"
              message="Add your five to the roster first — the builder picks agents from each player's own history."
            />
          </div>
        )}

        {setup.data && roster.length > 0 && (
          <>
            {/* Controls -------------------------------------------- */}
            <section className="card">
              <div className="card-head">
                <div className="grow">
                  <div className="card-title">Map</div>
                  <div className="card-sub">Pick where you are queueing</div>
                </div>
              </div>
              <div className="card-body">
                <div className="row wrap" style={{ gap: 8 }}>
                  {setup.data.catalog.maps.map((m) => (
                    <button
                      key={m.name}
                      className={`btn sm${activeMap === m.name ? ' primary' : ' ghost'}`}
                      onClick={() => setMap(m.name)}
                      style={!m.active && activeMap !== m.name ? { opacity: 0.55 } : undefined}
                      title={m.active ? 'In rotation' : 'Out of rotation'}
                    >
                      {m.name}
                    </button>
                  ))}
                </div>

                <div className="card-title" style={{ marginTop: 22, marginBottom: 10 }}>
                  Who is on
                </div>
                <div className="row wrap" style={{ gap: 8 }}>
                  {roster.map((p, i) => {
                    const on = selected.includes(p.player_id);
                    return (
                      <button
                        key={p.player_id}
                        className={`btn sm${on ? '' : ' ghost'}`}
                        onClick={() => togglePlayer(p.player_id)}
                        style={on ? { borderColor: 'rgba(255,255,255,0.3)' } : { opacity: 0.6 }}
                      >
                        <Avatar name={p.name} index={i} size="sm" />
                        {p.name}
                      </button>
                    );
                  })}
                </div>
              </div>
            </section>

            {build.loading && !result && <Loading rows={2} height={180} />}

            {result && result.slots.length > 0 && (
              <>
                <div className="grid cols-4">
                  <StatTile
                    label="Comp score"
                    value={pct(result.comp_score)}
                    accent="var(--s1)"
                    foot="Blend of win rate, impact, comfort and role fit"
                  />
                  <StatTile
                    label="Confidence"
                    value={pct(result.confidence)}
                    accent="var(--s3)"
                    foot="How much history backs these picks"
                  />
                  <StatTile
                    label={`Record on ${result.map}`}
                    value={result.map_record?.split(' ')[0] ?? '0–0'}
                    accent="var(--s4)"
                    foot={result.history ? `This exact comp: ${result.history.wins}–${result.history.played - result.history.wins}` : 'This exact comp is new'}
                  />
                  <div className="stat" style={{ ['--tile-accent' as string]: 'var(--s5)' }}>
                    <div className="stat-label">Role spread</div>
                    <div style={{ marginTop: 14 }}>
                      <RoleSpread spread={result.role_spread} />
                    </div>
                  </div>
                </div>

                {result.notes.map((n) => (
                  <div className="banner" key={n} style={{ borderColor: 'rgba(250,178,25,0.4)', background: 'rgba(250,178,25,0.12)', color: '#fac858' }}>
                    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                      <path d="M12 3 2 20h20L12 3Z" strokeLinejoin="round" />
                      <path d="M12 10v4M12 17v.5" strokeLinecap="round" />
                    </svg>
                    {n}
                  </div>
                ))}

                <div className="grid cols-3">
                  {result.slots.map((slot) => (
                    <div className={`comp-slot${slot.locked ? ' locked' : ''}`} key={slot.player_id}>
                      <div className="row between">
                        <div className="row" style={{ gap: 10 }}>
                          <Avatar
                            name={slot.player}
                            index={roster.findIndex((p) => p.player_id === slot.player_id)}
                            size="sm"
                          />
                          <div>
                            <div style={{ fontWeight: 600 }}>{slot.player}</div>
                            <div style={{ fontSize: 11, color: 'var(--ink-3)' }}>
                              prefers {slot.preferred_role}
                            </div>
                          </div>
                        </div>
                        {slot.locked && (
                          <button
                            className="pill accent"
                            style={{ cursor: 'pointer', border: 0 }}
                            onClick={() =>
                              setLocks((prev) => {
                                const copy = { ...prev };
                                delete copy[slot.player_id];
                                return copy;
                              })
                            }
                            title="Remove lock"
                          >
                            locked ✕
                          </button>
                        )}
                      </div>

                      <div>
                        <div className="comp-agent" style={{ color: roleColor(slot.role) }}>
                          {slot.agent}
                        </div>
                        <RoleTag role={slot.role} />
                      </div>

                      <div>
                        <div className="row between" style={{ fontSize: 11, color: 'var(--ink-3)', marginBottom: 5 }}>
                          <span>Fit</span>
                          <span className="num">{pct(slot.score)}</span>
                        </div>
                        <Meter value={slot.score} color={roleColor(slot.role)} />
                      </div>

                      <ul className="reason-list">
                        {slot.reasons.map((r) => (
                          <li key={r}>{r}</li>
                        ))}
                      </ul>

                      {slot.alternatives.length > 0 && (
                        <div>
                          <div style={{ fontSize: 10.5, letterSpacing: '0.14em', textTransform: 'uppercase', color: 'var(--ink-3)', marginBottom: 7 }}>
                            Swap to
                          </div>
                          <div className="grid" style={{ gap: 5 }}>
                            {slot.alternatives.slice(0, 3).map((alt) => (
                              <button
                                className="alt-chip"
                                key={alt.agent}
                                onClick={() =>
                                  setLocks((prev) => ({ ...prev, [slot.player_id]: alt.agent }))
                                }
                                title={`Lock ${slot.player} onto ${alt.agent} and rebuild the rest`}
                              >
                                <span className="row" style={{ gap: 7 }}>
                                  <span className={`role-dot role-${alt.role}`} />
                                  {alt.agent}
                                </span>
                                {/* Always lead with the same measure (fit), then
                                    add map history only where it exists. */}
                                <span className="num" style={{ color: 'var(--ink-3)' }}>
                                  {pct(alt.score)} fit
                                  {alt.picks_on_map > 0 && (
                                    <span style={{ opacity: 0.75 }}>
                                      {' · '}
                                      {pct(alt.win_rate_on_map ?? 0)} in {alt.picks_on_map}
                                    </span>
                                  )}
                                </span>
                              </button>
                            ))}
                          </div>
                        </div>
                      )}
                    </div>
                  ))}
                </div>

                <ChartFrame
                  title="Why these five"
                  subtitle="Each pick scores a player's own record on that agent, on this map, weighted by how much you have actually played it."
                >
                  <div className="table-wrap">
                    <table className="data">
                      <thead>
                        <tr>
                          <th>Player</th>
                          <th>Agent</th>
                          <th>Role</th>
                          <th>On {result.map}</th>
                          <th>Overall</th>
                          <th>K/D</th>
                          <th>ACS</th>
                          <th>Fit</th>
                        </tr>
                      </thead>
                      <tbody>
                        {result.slots.map((s) => (
                          <tr key={s.player_id}>
                            <td>{s.player}</td>
                            <td style={{ textAlign: 'right', color: roleColor(s.role), fontWeight: 600 }}>
                              {s.agent}
                            </td>
                            <td className="dim">{s.role}</td>
                            <td>
                              {s.picks_on_map > 0 ? `${pct(s.win_rate_on_map ?? 0)} (${s.picks_on_map})` : '—'}
                            </td>
                            <td>
                              {s.picks_overall > 0 ? `${pct(s.win_rate_overall ?? 0)} (${s.picks_overall})` : '—'}
                            </td>
                            <td>{s.kd !== null ? num(s.kd) : '—'}</td>
                            <td>{s.avg_acs !== null ? Math.round(s.avg_acs) : '—'}</td>
                            <td>{pct(s.score)}</td>
                          </tr>
                        ))}
                      </tbody>
                    </table>
                  </div>
                </ChartFrame>
              </>
            )}
          </>
        )}
      </div>
    </>
  );
}
