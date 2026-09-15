import { useRef, useState } from 'react';

import {
  api,
  type HenrikRequest,
  type ImportReport,
  type ProbeReport,
  type SourceInfo,
} from '../api';
import { useAsync } from '../hooks';
import { useFilters } from '../App';
import { TopBar } from '../components/Layout';
import { ErrorBanner, Loading, ResultPill } from '../components/Ui';

type Tab = 'file' | 'api' | 'backup';

/** Everything an import run can produce, rendered the same way each time. */
function ReportView({ report }: { report: ImportReport }) {
  const rows = report.preview.slice(0, 40);
  return (
    <div className="grid" style={{ gap: 14 }}>
      <div className="row wrap" style={{ gap: 8 }}>
        <span className={`pill ${report.imported > 0 ? 'win' : 'muted'}`}>
          {report.dry_run ? 'would import' : 'imported'} {report.imported}
        </span>
        {report.performances > 0 && (
          <span className="pill muted">{report.performances} scoreboard rows</span>
        )}
        {report.skipped_duplicates > 0 && (
          <span className="pill muted">{report.skipped_duplicates} already present</span>
        )}
        {report.skipped_empty > 0 && (
          <span className="pill muted">{report.skipped_empty} with nobody known</span>
        )}
        {report.created_players.length > 0 && (
          <span className="pill accent">
            {report.dry_run ? 'would add' : 'added'}: {report.created_players.join(', ')}
          </span>
        )}
      </div>

      {report.unknown_players.length > 0 && (
        <div className="banner" style={{ borderColor: 'rgba(250,178,25,0.4)', background: 'rgba(250,178,25,0.12)', color: '#fac858' }}>
          Not on the roster and not created, so their rows were dropped:{' '}
          {report.unknown_players.join(', ')}
        </div>
      )}
      {report.warnings.map((w) => (
        <div className="banner" key={w}>
          {w}
        </div>
      ))}

      {rows.length > 0 && (
        <div className="table-wrap">
          <table className="data">
            <thead>
              <tr>
                <th>Date</th>
                <th>Map</th>
                <th>Queue</th>
                <th>Score</th>
                <th>Players</th>
                <th>Status</th>
              </tr>
            </thead>
            <tbody>
              {rows.map((r, i) => (
                <tr key={`${r.played_at}-${r.map}-${i}`}>
                  <td className="dim">{r.played_at.slice(0, 16).replace('T', ' ')}</td>
                  <td>{r.map}</td>
                  <td className="dim">{r.mode}</td>
                  <td>
                    <span className="row" style={{ gap: 8, justifyContent: 'flex-end' }}>
                      <ResultPill result={r.result} />
                      <span className="num">{r.score}</span>
                    </span>
                  </td>
                  <td className="dim">{r.players}</td>
                  <td className={r.status === 'duplicate' ? 'dim' : undefined}>{r.status}</td>
                </tr>
              ))}
            </tbody>
          </table>
          {report.preview.length > rows.length && (
            <div className="card-sub" style={{ padding: '10px 18px' }}>
              …and {report.preview.length - rows.length} more
            </div>
          )}
        </div>
      )}
    </div>
  );
}

function ProbeView({ probe }: { probe: ProbeReport }) {
  return (
    <div className="grid" style={{ gap: 12 }}>
      <div className="row wrap" style={{ gap: 8 }}>
        <span className="pill muted">{probe.matches_seen} matches returned</span>
        <span className={`pill ${probe.missing.length === 0 ? 'win' : 'loss'}`}>
          {probe.resolved.length} fields mapped
        </span>
        {probe.missing.length > 0 && (
          <span className="pill loss">{probe.missing.length} unmapped</span>
        )}
      </div>

      {probe.notes.map((n) => (
        <div className="banner" key={n} style={{ borderColor: 'rgba(250,178,25,0.4)', background: 'rgba(250,178,25,0.12)', color: '#fac858' }}>
          {n}
        </div>
      ))}

      <div className="grid cols-2">
        <div>
          <div className="card-title" style={{ marginBottom: 8 }}>
            Mapped
          </div>
          <div className="grid" style={{ gap: 4 }}>
            {probe.resolved.map(([field, path]) => (
              <div className="row between" key={field} style={{ fontSize: 12.5 }}>
                <span style={{ color: 'var(--ink-2)' }}>{field}</span>
                <code style={{ color: 'var(--good-ink)' }}>{path}</code>
              </div>
            ))}
          </div>
        </div>
        <div>
          <div className="card-title" style={{ marginBottom: 8 }}>
            Keys the API actually returned
          </div>
          <div className="grid" style={{ gap: 8 }}>
            {probe.observed_keys.map(([level, keys]) => (
              <div key={level}>
                <div style={{ fontSize: 11, color: 'var(--ink-3)', letterSpacing: '0.1em', textTransform: 'uppercase' }}>
                  {level}
                </div>
                <div style={{ fontSize: 12, color: 'var(--ink-2)', wordBreak: 'break-word' }}>
                  {keys.join(', ')}
                </div>
              </div>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
}

export default function ImportPage() {
  const { invalidate, version } = useFilters();
  const [tab, setTab] = useState<Tab>('file');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [report, setReport] = useState<ImportReport | null>(null);
  const [probe, setProbe] = useState<ProbeReport | null>(null);

  /** Results belong to the tab that produced them — carrying a file import's
   *  report over to the API tab reads as if the API had produced it. */
  const switchTab = (next: Tab) => {
    setTab(next);
    setReport(null);
    setProbe(null);
    setError(null);
  };

  const [text, setText] = useState('');
  const [fileName, setFileName] = useState<string | null>(null);
  const fileInput = useRef<HTMLInputElement>(null);

  const [henrik, setHenrik] = useState<HenrikRequest>({
    name: '',
    region: 'eu',
    platform: 'pc',
    mode: 'Competitive',
    size: 10,
  });

  const sources = useAsync<SourceInfo[]>(async () => (await api.sources()).sources, [version]);
  const henrikSource = sources.data?.find((s) => s.id === 'henrikdev');

  const looksLikeJson = text.trim().startsWith('{') || text.trim().startsWith('[');

  const run = async (dryRun: boolean) => {
    setBusy(true);
    setError(null);
    setProbe(null);
    try {
      let result: ImportReport;
      if (tab === 'file') {
        const options = { dry_run: dryRun };
        result = looksLikeJson
          ? await api.importJson(JSON.parse(text), options)
          : await api.importCsv(text, options);
      } else {
        const res = await api.henrikSync({ ...henrik, options: { dry_run: dryRun } });
        result = res.report;
        setProbe(res.probe);
      }
      setReport(result);
      if (!dryRun && result.imported > 0) invalidate();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Import failed');
      setReport(null);
    } finally {
      setBusy(false);
    }
  };

  const runProbe = async () => {
    setBusy(true);
    setError(null);
    setReport(null);
    try {
      const res = await api.henrikProbe(henrik);
      setProbe(res.probe);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Probe failed');
    } finally {
      setBusy(false);
    }
  };

  const onFile = async (file: File) => {
    setFileName(file.name);
    setText(await file.text());
    setReport(null);
    setError(null);
  };

  return (
    <>
      <TopBar title="Import" subtitle="Bring history in from a file or an API">
        <div className="seg">
          <button aria-pressed={tab === 'file'} onClick={() => switchTab('file')}>
            File
          </button>
          <button aria-pressed={tab === 'api'} onClick={() => switchTab('api')}>
            Riot ID
          </button>
          <button aria-pressed={tab === 'backup'} onClick={() => switchTab('backup')}>
            Backup
          </button>
        </div>
      </TopBar>

      <div className="page fade-in">
        {error && <ErrorBanner message={error} />}
        {sources.loading && !sources.data && <Loading rows={1} height={120} />}

        {/* ---------------------------------------------------- file ---- */}
        {tab === 'file' && (
          <section className="card">
            <div className="card-head">
              <div className="grow">
                <div className="card-title">From a file</div>
                <div className="card-sub">
                  A valops JSON export, or a CSV where each row is one player in one match.
                  Nothing is written until you have seen the preview.
                </div>
              </div>
              <a className="btn sm ghost" href="/api/import/template.csv" download>
                CSV template
              </a>
            </div>
            <div className="card-body">
              <div className="row wrap" style={{ marginBottom: 12 }}>
                <input
                  ref={fileInput}
                  type="file"
                  accept=".json,.csv,text/csv,application/json"
                  style={{ display: 'none' }}
                  onChange={(e) => {
                    const f = e.target.files?.[0];
                    if (f) void onFile(f);
                  }}
                />
                <button className="btn" onClick={() => fileInput.current?.click()}>
                  Choose a file
                </button>
                {fileName && <span className="tag">{fileName}</span>}
                {text && (
                  <span className="tag">{looksLikeJson ? 'reading as JSON' : 'reading as CSV'}</span>
                )}
              </div>

              <div className="field">
                <label htmlFor="paste">…or paste it here</label>
                <textarea
                  id="paste"
                  className="input"
                  rows={8}
                  spellCheck={false}
                  style={{ fontFamily: 'ui-monospace, SFMono-Regular, Menlo, monospace', fontSize: 12.5, resize: 'vertical' }}
                  placeholder={'played_at,map,mode,rounds_won,rounds_lost,player,agent,kills,deaths,assists,acs\n2026-09-15T21:30:00,Ascent,Competitive,13,9,Vex,Jett,22,15,4,271'}
                  value={text}
                  onChange={(e) => {
                    setText(e.target.value);
                    setReport(null);
                  }}
                />
              </div>

              <div className="row" style={{ marginTop: 14 }}>
                <button className="btn" onClick={() => run(true)} disabled={busy || !text.trim()}>
                  {busy ? 'Working…' : 'Preview'}
                </button>
                <button
                  className="btn primary"
                  onClick={() => run(false)}
                  disabled={busy || !text.trim() || !report?.dry_run || report.imported === 0}
                  title={report?.dry_run ? undefined : 'Preview first'}
                >
                  Import {report?.dry_run ? report.imported : ''} match
                  {report?.dry_run && report.imported === 1 ? '' : 'es'}
                </button>
              </div>
            </div>
          </section>
        )}

        {/* ----------------------------------------------------- api ---- */}
        {tab === 'api' && (
          <section className="card">
            <div className="card-head">
              <div className="grow">
                <div className="card-title">Pull from a Riot ID</div>
                <div className="card-sub">
                  Fetches recent matches through the HenrikDev API and keeps the rows for players
                  on your roster.
                </div>
              </div>
            </div>
            <div className="card-body">
              {henrikSource && !henrikSource.configured && (
                <div className="banner" style={{ marginBottom: 16 }}>
                  No API key set. {henrikSource.setup}
                </div>
              )}
              {henrikSource && !henrikSource.verified && (
                <div
                  className="banner"
                  style={{ marginBottom: 16, borderColor: 'rgba(250,178,25,0.4)', background: 'rgba(250,178,25,0.12)', color: '#fac858' }}
                >
                  This source's field mapping has never been run against the live API. Use{' '}
                  <strong style={{ margin: '0 4px' }}>Run probe</strong> first — it fetches without
                  importing and reports which fields it could read.
                </div>
              )}

              <div className="grid cols-4">
                <div className="field">
                  <label htmlFor="riot">Riot ID</label>
                  <input
                    id="riot"
                    className="input"
                    placeholder="Vex#EUW"
                    value={henrik.name}
                    onChange={(e) => setHenrik({ ...henrik, name: e.target.value })}
                  />
                </div>
                <div className="field">
                  <label htmlFor="region">Region</label>
                  <select
                    id="region"
                    className="select"
                    value={henrik.region}
                    onChange={(e) => setHenrik({ ...henrik, region: e.target.value })}
                  >
                    {(henrikSource?.regions ?? ['eu', 'na', 'latam', 'br', 'ap', 'kr']).map((r) => (
                      <option key={r} value={r}>
                        {r.toUpperCase()}
                      </option>
                    ))}
                  </select>
                </div>
                <div className="field">
                  <label htmlFor="queue">Queue</label>
                  <select
                    id="queue"
                    className="select"
                    value={henrik.mode}
                    onChange={(e) => setHenrik({ ...henrik, mode: e.target.value })}
                  >
                    {['Competitive', 'Premier', 'Unrated', 'Swiftplay', ''].map((m) => (
                      <option key={m || 'any'} value={m}>
                        {m || 'Any queue'}
                      </option>
                    ))}
                  </select>
                </div>
                <div className="field">
                  <label htmlFor="size">Matches</label>
                  <input
                    id="size"
                    className="input"
                    type="number"
                    min={1}
                    max={20}
                    value={henrik.size}
                    onChange={(e) => setHenrik({ ...henrik, size: Number(e.target.value) })}
                  />
                </div>
              </div>

              <div className="row wrap" style={{ marginTop: 16 }}>
                <button
                  className="btn ghost"
                  onClick={runProbe}
                  disabled={busy || !henrik.name.trim() || !henrikSource?.configured}
                >
                  Run probe
                </button>
                <button
                  className="btn"
                  onClick={() => run(true)}
                  disabled={busy || !henrik.name.trim() || !henrikSource?.configured}
                >
                  {busy ? 'Working…' : 'Preview'}
                </button>
                <button
                  className="btn primary"
                  onClick={() => run(false)}
                  disabled={busy || !report?.dry_run || report.imported === 0}
                >
                  Import {report?.dry_run ? report.imported : ''}
                </button>
              </div>
            </div>
          </section>
        )}

        {/* -------------------------------------------------- backup ---- */}
        {tab === 'backup' && (
          <section className="card">
            <div className="card-head">
              <div className="grow">
                <div className="card-title">Backup and restore</div>
                <div className="card-sub">
                  The export is a complete copy of your roster and match history, and it restores
                  through the File tab.
                </div>
              </div>
            </div>
            <div className="card-body">
              <div className="row wrap">
                <a className="btn primary" href="/api/export" download="valops-export.json">
                  Download export
                </a>
                <button className="btn ghost" onClick={() => switchTab('file')}>
                  Restore from a file
                </button>
              </div>
              <p style={{ color: 'var(--ink-3)', fontSize: 13, marginBottom: 0 }}>
                Restoring is safe to repeat: a match that already exists is skipped rather than
                added twice.
              </p>
            </div>
          </section>
        )}

        {report && (
          <section className="card">
            <div className="card-head">
              <div className="grow">
                <div className="card-title">{report.dry_run ? 'Preview' : 'Result'}</div>
                <div className="card-sub">
                  {report.dry_run
                    ? 'Nothing has been written yet.'
                    : 'Written to the database.'}
                </div>
              </div>
            </div>
            <div className="card-body">
              <ReportView report={report} />
            </div>
          </section>
        )}

        {probe && (
          <section className="card">
            <div className="card-head">
              <div className="grow">
                <div className="card-title">Probe</div>
                <div className="card-sub">
                  What the mapper could read from the API's response.
                </div>
              </div>
            </div>
            <div className="card-body">
              <ProbeView probe={probe} />
            </div>
          </section>
        )}
      </div>
    </>
  );
}
