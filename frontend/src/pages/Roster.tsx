import { useState } from 'react';

import { api, type Catalog, type Player, type PlayerInput, type PlayerStat } from '../api';
import { useAsync } from '../hooks';
import { useFilters } from '../App';
import { TopBar, WindowPicker } from '../components/Layout';
import { BarChart, LineChart } from '../components/Charts';
import {
  Avatar,
  CellBar,
  Empty,
  ErrorBanner,
  Loading,
  Meter,
  Modal,
  RoleTag,
  StatTile,
} from '../components/Ui';
import { num, pct, seriesColor, shortDate, signed } from '../lib';

interface Bundle {
  stats: PlayerStat[];
  players: Player[];
  catalog: Catalog;
}

const emptyForm: PlayerInput = { name: '', riot_id: '', role: 'Flex', rank: '', active: true };

export default function Roster() {
  const { days, setDays, version, invalidate } = useFilters();
  const [editing, setEditing] = useState<{ id: number | null; form: PlayerInput } | null>(null);
  const [detailId, setDetailId] = useState<number | null>(null);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  const { data, error, loading, reload } = useAsync<Bundle>(async () => {
    const [stats, players, catalog] = await Promise.all([
      api.playerStats({ days }),
      api.players(),
      api.catalog(),
    ]);
    return { stats, players, catalog };
  }, [days, version]);

  const save = async () => {
    if (!editing) return;
    setSaving(true);
    setSaveError(null);
    try {
      if (editing.id === null) await api.createPlayer(editing.form);
      else await api.updatePlayer(editing.id, editing.form);
      setEditing(null);
      invalidate();
    } catch (err) {
      setSaveError(err instanceof Error ? err.message : 'Could not save');
    } finally {
      setSaving(false);
    }
  };

  const remove = async (id: number) => {
    if (!window.confirm('Remove this player and every scoreboard row they appear in?')) return;
    try {
      await api.deletePlayer(id);
      setEditing(null);
      setDetailId(null);
      invalidate();
    } catch (err) {
      setSaveError(err instanceof Error ? err.message : 'Could not delete');
    }
  };

  const detail = data?.stats.find((p) => p.player_id === detailId) ?? null;
  const detailIndex = data?.stats.findIndex((p) => p.player_id === detailId) ?? 0;

  return (
    <>
      <TopBar title="Roster" subtitle="Who plays what, and how well it goes">
        <WindowPicker days={days} onChange={setDays} />
        <button className="btn primary" onClick={() => setEditing({ id: null, form: { ...emptyForm } })}>
          Add player
        </button>
      </TopBar>

      <div className="page fade-in">
        {error && <ErrorBanner message={error} onRetry={reload} />}
        {saveError && <ErrorBanner message={saveError} />}
        {!data && loading && <Loading rows={2} height={160} />}

        {data && data.stats.length === 0 && (
          <div className="card">
            <Empty
              title="No players yet"
              message="Add the five of you. Names are all that is required — Riot ID and role just make the builder smarter."
              action={
                <button className="btn primary" onClick={() => setEditing({ id: null, form: { ...emptyForm } })}>
                  Add the first player
                </button>
              }
            />
          </div>
        )}

        {data && data.stats.length > 0 && (
          <div className="grid cols-3">
            {data.stats.map((p, i) => (
              <article
                className="player-card"
                key={p.player_id}
                onClick={() => setDetailId(p.player_id)}
                style={{ cursor: 'pointer', opacity: p.active ? 1 : 0.6 }}
              >
                <div className="row" style={{ gap: 13 }}>
                  <Avatar name={p.name} index={i} />
                  <div style={{ minWidth: 0, flex: 1 }}>
                    <div className="row between">
                      <strong style={{ fontSize: 15 }}>{p.name}</strong>
                      {!p.active && <span className="pill muted">benched</span>}
                    </div>
                    <div className="row" style={{ gap: 10, marginTop: 3 }}>
                      <RoleTag role={p.role} />
                      {p.rank && <span style={{ fontSize: 11.5, color: 'var(--ink-3)' }}>{p.rank}</span>}
                    </div>
                  </div>
                </div>

                <div className="row" style={{ gap: 18 }}>
                  <div className="kv">
                    <span className="k">K/D</span>
                    <span className="v">{num(p.kd)}</span>
                  </div>
                  <div className="kv">
                    <span className="k">ACS</span>
                    <span className="v">{Math.round(p.avg_acs) || '—'}</span>
                  </div>
                  {/* Not win rate: everyone in a five-stack shares it. KDA
                      is the number that actually separates players. */}
                  <div className="kv">
                    <span className="k">KDA</span>
                    <span className="v">{p.matches ? num(p.kda) : '—'}</span>
                  </div>
                  <div className="kv">
                    <span className="k">Entries</span>
                    <span className="v" style={{ color: p.entry_diff >= 0 ? 'var(--good-ink)' : 'var(--critical-ink)' }}>
                      {signed(p.entry_diff)}
                    </span>
                  </div>
                </div>

                <div>
                  <div className="row between" style={{ fontSize: 11, color: 'var(--ink-3)', marginBottom: 5 }}>
                    <span>Rating</span>
                    <span className="num">{pct(p.rating)}</span>
                  </div>
                  <Meter value={p.rating} color={seriesColor(i)} />
                </div>

                <div className="row wrap" style={{ gap: 6 }}>
                  {p.best_agents.slice(0, 3).map((a) => (
                    <span className="tag" key={a.agent}>
                      {a.agent} · {a.picks}× · {pct(a.win_rate)}
                    </span>
                  ))}
                  {p.best_agents.length === 0 && <span className="tag">No matches logged</span>}
                </div>
              </article>
            ))}
          </div>
        )}
      </div>

      {/* Player detail ------------------------------------------------ */}
      {detail && (
        <Modal
          title={detail.name}
          onClose={() => setDetailId(null)}
          footer={
            <>
              <button className="btn danger" onClick={() => remove(detail.player_id)}>
                Remove player
              </button>
              <button
                className="btn"
                onClick={() => {
                  const source = data?.players.find((p) => p.id === detail.player_id);
                  setDetailId(null);
                  setEditing({
                    id: detail.player_id,
                    form: {
                      name: source?.name ?? detail.name,
                      riot_id: source?.riot_id ?? '',
                      role: source?.role ?? detail.role,
                      rank: source?.rank ?? '',
                      active: source?.active ?? true,
                    },
                  });
                }}
              >
                Edit
              </button>
              <button className="btn ghost" onClick={() => setDetailId(null)}>
                Close
              </button>
            </>
          }
        >
          <div className="row" style={{ gap: 14 }}>
            <Avatar name={detail.name} index={detailIndex} />
            <div>
              <div className="row" style={{ gap: 10 }}>
                <RoleTag role={detail.role} />
                {detail.rank && <span className="tag">{detail.rank}</span>}
                {detail.riot_id && <span className="tag">{detail.riot_id}</span>}
              </div>
              <div className="card-sub" style={{ marginTop: 4 }}>
                {detail.matches} matches · {detail.agent_pool} agents played
              </div>
            </div>
          </div>

          <div className="grid cols-4">
            <StatTile label="K/D" value={num(detail.kd)} accent="var(--s1)" foot={`${detail.kills}/${detail.deaths}`} />
            <StatTile label="KDA" value={num(detail.kda)} accent="var(--s3)" foot={`${detail.assists} assists`} />
            <StatTile label="Avg ACS" value={Math.round(detail.avg_acs)} accent="var(--s4)" foot={`${num(detail.avg_kills)} kills per match`} />
            <StatTile
              label="Entry duels"
              value={signed(detail.entry_diff)}
              accent="var(--s5)"
              foot={`${detail.first_bloods} first bloods · ${detail.first_deaths} first deaths`}
            />
          </div>

          {detail.best_agents.length > 0 && (
            <div>
              <div className="card-title" style={{ marginBottom: 10 }}>
                Agent win rate
              </div>
              <BarChart
                data={detail.best_agents.map((a) => ({
                  label: a.agent,
                  value: a.win_rate,
                  rows: [
                    { label: 'Picks', value: String(a.picks) },
                    { label: 'K/D', value: num(a.kd) },
                    { label: 'ACS', value: String(Math.round(a.avg_acs)) },
                  ],
                }))}
                format={(v) => pct(v)}
                baseline={0.5}
                domain={[0, 1]}
                valueLabel="Win rate"
                labelWidth={84}
              />
            </div>
          )}

          {detail.best_maps.length > 0 && (
            <div>
              <div className="card-title" style={{ marginBottom: 10 }}>
                Map win rate
              </div>
              <div className="table-wrap">
                <table className="data">
                  <thead>
                    <tr>
                      <th>Map</th>
                      <th>Played</th>
                      <th>Win rate</th>
                      <th>K/D</th>
                      <th>ACS</th>
                    </tr>
                  </thead>
                  <tbody>
                    {detail.best_maps.map((m) => (
                      <tr key={m.map}>
                        <td>{m.map}</td>
                        <td className="dim">{m.played}</td>
                        <td>
                          <CellBar value={m.win_rate} color={seriesColor(detailIndex)} />{' '}
                          <span className="num">{pct(m.win_rate)}</span>
                        </td>
                        <td>{num(m.kd)}</td>
                        <td>{Math.round(m.avg_acs)}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </div>
          )}

          {detail.timeline.length > 2 && (
            <div>
              <div className="card-title" style={{ marginBottom: 10 }}>
                K/D per match
              </div>
              <LineChart
                series={[
                  {
                    name: 'K/D',
                    color: seriesColor(detailIndex),
                    points: detail.timeline.slice(-40).map((t) => ({
                      x: t.played_at,
                      y: t.kd,
                      label: `${shortDate(t.played_at)} · ${t.map} · ${t.agent}`,
                    })),
                  },
                ]}
                yFormat={(v) => v.toFixed(1)}
                reference={1}
                height={190}
              />
            </div>
          )}
        </Modal>
      )}

      {/* Add / edit --------------------------------------------------- */}
      {editing && (
        <Modal
          title={editing.id === null ? 'Add player' : 'Edit player'}
          onClose={() => setEditing(null)}
          footer={
            <>
              <button className="btn ghost" onClick={() => setEditing(null)}>
                Cancel
              </button>
              <button className="btn primary" onClick={save} disabled={saving || !editing.form.name.trim()}>
                {saving ? 'Saving…' : 'Save'}
              </button>
            </>
          }
        >
          <div className="grid cols-2">
            <div className="field">
              <label htmlFor="pname">Name</label>
              <input
                id="pname"
                className="input"
                value={editing.form.name}
                autoFocus
                placeholder="Vex"
                onChange={(e) => setEditing({ ...editing, form: { ...editing.form, name: e.target.value } })}
              />
            </div>
            <div className="field">
              <label htmlFor="priot">Riot ID</label>
              <input
                id="priot"
                className="input"
                value={editing.form.riot_id ?? ''}
                placeholder="Vex#EUW"
                onChange={(e) => setEditing({ ...editing, form: { ...editing.form, riot_id: e.target.value } })}
              />
            </div>
            <div className="field">
              <label htmlFor="prole">Preferred role</label>
              <select
                id="prole"
                className="select"
                value={editing.form.role}
                onChange={(e) => setEditing({ ...editing, form: { ...editing.form, role: e.target.value } })}
              >
                {['Duelist', 'Initiator', 'Controller', 'Sentinel', 'Flex'].map((r) => (
                  <option key={r} value={r}>
                    {r}
                  </option>
                ))}
              </select>
            </div>
            <div className="field">
              <label htmlFor="prank">Rank</label>
              <input
                id="prank"
                className="input"
                value={editing.form.rank ?? ''}
                placeholder="Ascendant 2"
                onChange={(e) => setEditing({ ...editing, form: { ...editing.form, rank: e.target.value } })}
              />
            </div>
          </div>
          <label className="row" style={{ gap: 9, cursor: 'pointer' }}>
            <input
              type="checkbox"
              checked={editing.form.active}
              onChange={(e) => setEditing({ ...editing, form: { ...editing.form, active: e.target.checked } })}
            />
            <span>In the active five (the builder only picks from active players)</span>
          </label>
        </Modal>
      )}
    </>
  );
}
