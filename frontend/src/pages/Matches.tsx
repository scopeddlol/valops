import { useMemo, useState } from 'react';

import {
  api,
  type Catalog,
  type MatchDetail,
  type MatchInput,
  type PerformanceInput,
  type PlayerStat,
} from '../api';
import { useAsync } from '../hooks';
import { useFilters } from '../App';
import { TopBar } from '../components/Layout';
import { Empty, ErrorBanner, Loading, Modal, ResultPill } from '../components/Ui';
import { fullDate, localDateTimeValue, num, pct, roleColor, shortDate } from '../lib';

interface Bundle {
  matches: MatchDetail[];
  players: PlayerStat[];
  catalog: Catalog;
}

const blankRow = (player_id: number, agent: string): PerformanceInput => ({
  player_id,
  agent,
  kills: 0,
  deaths: 0,
  assists: 0,
  acs: 0,
  first_bloods: 0,
  first_deaths: 0,
  plants: 0,
  defuses: 0,
});

export default function Matches() {
  const { version, invalidate } = useFilters();
  const [open, setOpen] = useState<number | null>(null);
  const [form, setForm] = useState<MatchInput | null>(null);
  const [saving, setSaving] = useState(false);
  const [formError, setFormError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const { data, error, loading, reload } = useAsync<Bundle>(async () => {
    const [matches, players, catalog] = await Promise.all([
      api.matches(300),
      api.playerStats(),
      api.catalog(),
    ]);
    return { matches, players, catalog };
  }, [version]);

  const agentsByRole = useMemo(() => {
    const groups = new Map<string, string[]>();
    for (const a of data?.catalog.agents ?? []) {
      if (!groups.has(a.role)) groups.set(a.role, []);
      groups.get(a.role)!.push(a.name);
    }
    return groups;
  }, [data]);

  const startLog = () => {
    const active = (data?.players ?? []).filter((p) => p.active);
    setFormError(null);
    setForm({
      played_at: localDateTimeValue(),
      // You usually log right after playing, so the last map is the best guess.
      map: data?.matches[0]?.map ?? data?.catalog.maps.find((m) => m.active)?.name ?? 'Ascent',
      mode: 'Competitive',
      rounds_won: 13,
      rounds_lost: 0,
      notes: '',
      // Pre-fill each player on the agent they play most — most nights that is
      // already right, and it makes logging a match a few keystrokes.
      performances: active.map((p) =>
        blankRow(p.player_id, p.best_agents[0]?.agent ?? data?.catalog.agents[0]?.name ?? 'Jett'),
      ),
    });
  };

  const setRow = (index: number, patch: Partial<PerformanceInput>) => {
    setForm((f) =>
      f
        ? {
            ...f,
            performances: f.performances.map((row, i) => (i === index ? { ...row, ...patch } : row)),
          }
        : f,
    );
  };

  const submit = async () => {
    if (!form) return;
    setSaving(true);
    setFormError(null);
    try {
      await api.createMatch(form);
      setForm(null);
      invalidate();
    } catch (err) {
      setFormError(err instanceof Error ? err.message : 'Could not save the match');
    } finally {
      setSaving(false);
    }
  };

  const removeMatch = async (id: number) => {
    if (!window.confirm('Delete this match and its scoreboard?')) return;
    await api.deleteMatch(id);
    setOpen(null);
    invalidate();
  };

  const runDemo = async (mode: 'seed' | 'reset') => {
    const message =
      mode === 'seed'
        ? 'Replace everything with a generated demo history? Any matches you logged will be deleted.'
        : 'Delete every player and match? This cannot be undone.';
    if (!window.confirm(message)) return;
    setBusy(true);
    try {
      if (mode === 'seed') await api.seedDemo();
      else await api.resetAll();
      invalidate();
    } finally {
      setBusy(false);
    }
  };

  const playerName = (id: number) => data?.players.find((p) => p.player_id === id)?.name ?? `#${id}`;

  return (
    <>
      <TopBar
        title="Matches"
        subtitle={data ? `${data.matches.length} logged` : 'Loading…'}
      >
        <a className="btn ghost" href="/api/export" download="valops-export.json">
          Export JSON
        </a>
        <button className="btn primary" onClick={startLog} disabled={!data}>
          Log match
        </button>
      </TopBar>

      <div className="page fade-in">
        {error && <ErrorBanner message={error} onRetry={reload} />}
        {!data && loading && <Loading rows={4} height={64} />}

        {data && data.matches.length === 0 && (
          <div className="card">
            <Empty
              title="No matches yet"
              message="Log one by hand, or drop in a generated six-month history to explore what the app does with volume."
              action={
                <div className="row" style={{ marginTop: 6 }}>
                  <button className="btn primary" onClick={startLog}>
                    Log a match
                  </button>
                  <button className="btn ghost" onClick={() => runDemo('seed')} disabled={busy}>
                    Load demo history
                  </button>
                </div>
              }
            />
          </div>
        )}

        {data && data.matches.length > 0 && (
          <section className="card">
            <div className="card-head">
              <div className="grow">
                <div className="card-title">Match log</div>
                <div className="card-sub">Newest first — click a row for the scoreboard</div>
              </div>
            </div>
            <div className="card-body flush">
              {data.matches.map((m) => (
                <div key={m.id}>
                  <div
                    className={`match-row${open === m.id ? ' open' : ''}`}
                    onClick={() => setOpen(open === m.id ? null : m.id)}
                  >
                    <ResultPill result={m.result} />
                    <div>
                      <div style={{ fontWeight: 600 }}>{m.map}</div>
                      <div style={{ fontSize: 11.5, color: 'var(--ink-3)' }}>{shortDate(m.played_at)}</div>
                    </div>
                    <div className="score-line">
                      <span style={{ color: m.result === 'win' ? 'var(--good-ink)' : undefined }}>
                        {m.rounds_won}
                      </span>
                      <span style={{ color: 'var(--ink-3)' }}> – </span>
                      <span className={m.result === 'win' ? 'lose' : ''}>{m.rounds_lost}</span>
                    </div>
                    <div className="row wrap hide-sm" style={{ gap: 6 }}>
                      {m.performances.slice(0, 5).map((p) => (
                        <span className="tag" key={p.id} style={{ borderColor: 'transparent' }}>
                          <span
                            className="role-dot"
                            style={{ background: roleColor(p.agent_role), marginRight: 6 }}
                          />
                          {p.agent}
                        </span>
                      ))}
                    </div>
                    <div className="num hide-sm" style={{ textAlign: 'right', color: 'var(--ink-3)', fontSize: 12.5 }}>
                      {m.performances.length > 0
                        ? `${m.performances[0].player_name} ${m.performances[0].acs}`
                        : '—'}
                    </div>
                  </div>

                  {open === m.id && (
                    <div className="match-detail fade-in">
                      <div className="table-wrap">
                        <table className="data">
                          <thead>
                            <tr>
                              <th>Player</th>
                              <th>Agent</th>
                              <th>ACS</th>
                              <th>K</th>
                              <th>D</th>
                              <th>A</th>
                              <th>K/D</th>
                              <th>FB</th>
                              <th>FD</th>
                              <th>Plants</th>
                              <th>Defuses</th>
                            </tr>
                          </thead>
                          <tbody>
                            {m.performances.map((p) => (
                              <tr key={p.id}>
                                <td>{p.player_name}</td>
                                <td style={{ color: roleColor(p.agent_role), fontWeight: 600 }}>{p.agent}</td>
                                <td>{p.acs}</td>
                                <td>{p.kills}</td>
                                <td>{p.deaths}</td>
                                <td>{p.assists}</td>
                                <td>{num(p.deaths ? p.kills / p.deaths : p.kills)}</td>
                                <td className="dim">{p.first_bloods}</td>
                                <td className="dim">{p.first_deaths}</td>
                                <td className="dim">{p.plants}</td>
                                <td className="dim">{p.defuses}</td>
                              </tr>
                            ))}
                          </tbody>
                        </table>
                      </div>
                      <div className="row between" style={{ marginTop: 14 }}>
                        <span style={{ fontSize: 12.5, color: 'var(--ink-3)' }}>
                          {fullDate(m.played_at)} · {m.mode}
                          {m.notes ? ` · ${m.notes}` : ''}
                        </span>
                        <button className="btn sm danger" onClick={() => removeMatch(m.id)}>
                          Delete match
                        </button>
                      </div>
                    </div>
                  )}
                </div>
              ))}
            </div>
          </section>
        )}

        {data && (
          <section className="card">
            <div className="card-head">
              <div className="grow">
                <div className="card-title">Data</div>
                <div className="card-sub">Everything lives in the SQLite file on your volume</div>
              </div>
            </div>
            <div className="card-body">
              <div className="row wrap">
                <button className="btn ghost" onClick={() => runDemo('seed')} disabled={busy}>
                  Regenerate demo history
                </button>
                <button className="btn danger" onClick={() => runDemo('reset')} disabled={busy}>
                  Delete all data
                </button>
              </div>
            </div>
          </section>
        )}
      </div>

      {/* Log match --------------------------------------------------- */}
      {form && data && (
        <Modal
          title="Log a match"
          onClose={() => setForm(null)}
          footer={
            <>
              <button className="btn ghost" onClick={() => setForm(null)}>
                Cancel
              </button>
              <button className="btn primary" onClick={submit} disabled={saving}>
                {saving ? 'Saving…' : 'Save match'}
              </button>
            </>
          }
        >
          {formError && <ErrorBanner message={formError} />}

          <div className="grid cols-4">
            <div className="field">
              <label htmlFor="when">Played at</label>
              <input
                id="when"
                type="datetime-local"
                className="input"
                value={form.played_at}
                onChange={(e) => setForm({ ...form, played_at: e.target.value })}
              />
            </div>
            <div className="field">
              <label htmlFor="map">Map</label>
              <select
                id="map"
                className="select"
                value={form.map}
                onChange={(e) => setForm({ ...form, map: e.target.value })}
              >
                {data.catalog.maps.map((m) => (
                  <option key={m.name} value={m.name}>
                    {m.name}
                    {m.active ? '' : ' (out of pool)'}
                  </option>
                ))}
              </select>
            </div>
            <div className="field">
              <label htmlFor="mode">Queue</label>
              <select
                id="mode"
                className="select"
                value={form.mode}
                onChange={(e) => setForm({ ...form, mode: e.target.value })}
              >
                {['Competitive', 'Premier', 'Unrated', 'Swiftplay', 'Custom'].map((m) => (
                  <option key={m}>{m}</option>
                ))}
              </select>
            </div>
            <div className="field">
              <label>Final score</label>
              <div className="row" style={{ gap: 8 }}>
                <input
                  className="input num-input"
                  type="number"
                  min={0}
                  aria-label="Rounds won"
                  value={form.rounds_won}
                  onChange={(e) => setForm({ ...form, rounds_won: Number(e.target.value) })}
                />
                <span style={{ color: 'var(--ink-3)' }}>–</span>
                <input
                  className="input num-input"
                  type="number"
                  min={0}
                  aria-label="Rounds lost"
                  value={form.rounds_lost}
                  onChange={(e) => setForm({ ...form, rounds_lost: Number(e.target.value) })}
                />
              </div>
            </div>
          </div>

          <div>
            <div className="card-title" style={{ marginBottom: 10 }}>
              Scoreboard
            </div>
            <div className="table-wrap">
              <table className="data">
                <thead>
                  <tr>
                    <th>Player</th>
                    <th>Agent</th>
                    <th>ACS</th>
                    <th>K</th>
                    <th>D</th>
                    <th>A</th>
                    <th>FB</th>
                    <th>FD</th>
                    <th>Plants</th>
                    <th>Defuses</th>
                  </tr>
                </thead>
                <tbody>
                  {form.performances.map((row, i) => (
                    <tr key={row.player_id}>
                      <td style={{ fontWeight: 600 }}>{playerName(row.player_id)}</td>
                      <td style={{ minWidth: 150 }}>
                        <select
                          className="select"
                          value={row.agent}
                          aria-label={`Agent for ${playerName(row.player_id)}`}
                          onChange={(e) => setRow(i, { agent: e.target.value })}
                        >
                          {[...agentsByRole.entries()].map(([groupRole, names]) => (
                            <optgroup label={groupRole} key={groupRole}>
                              {names.map((n) => (
                                <option key={n} value={n}>
                                  {n}
                                </option>
                              ))}
                            </optgroup>
                          ))}
                        </select>
                      </td>
                      {(
                        [
                          'acs',
                          'kills',
                          'deaths',
                          'assists',
                          'first_bloods',
                          'first_deaths',
                          'plants',
                          'defuses',
                        ] as const
                      ).map((field) => (
                        <td key={field} style={{ width: 66 }}>
                          <input
                            className="input num-input"
                            type="number"
                            min={0}
                            aria-label={`${field} for ${playerName(row.player_id)}`}
                            value={row[field]}
                            onChange={(e) => setRow(i, { [field]: Number(e.target.value) })}
                          />
                        </td>
                      ))}
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
            {form.performances.length === 0 && (
              <p style={{ color: 'var(--ink-3)', fontSize: 13 }}>
                No active players on the roster — add players first and their rows appear here.
              </p>
            )}
          </div>

          <div className="field">
            <label htmlFor="notes">Notes</label>
            <input
              id="notes"
              className="input"
              placeholder="Threw a 10-3 lead, retake default wasn't working"
              value={form.notes ?? ''}
              onChange={(e) => setForm({ ...form, notes: e.target.value })}
            />
          </div>

          <div className="row" style={{ color: 'var(--ink-3)', fontSize: 12.5 }}>
            Result:{' '}
            <strong style={{ color: form.rounds_won > form.rounds_lost ? 'var(--good-ink)' : 'var(--critical-ink)' }}>
              {form.rounds_won > form.rounds_lost ? 'win' : form.rounds_won < form.rounds_lost ? 'loss' : 'draw'}
            </strong>{' '}
            · round win rate{' '}
            {pct(
              form.rounds_won + form.rounds_lost > 0
                ? form.rounds_won / (form.rounds_won + form.rounds_lost)
                : 0,
            )}
          </div>
        </Modal>
      )}
    </>
  );
}
