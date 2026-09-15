/**
 * Hand-rolled SVG charts.
 *
 * Deliberately not a charting library: every mark here is specified — thin
 * marks, rounded data-ends anchored to the baseline, recessive grid lines, a
 * hover layer on every plot, and colour assigned by the job it does (diverging
 * for win rate around an even 50%, one-hue sequential for magnitude).
 */

import { useMemo, useState, type ReactNode } from 'react';
import { useMeasure } from '../hooks';
import { clamp, mix, sequentialBlue, winRateColor } from '../lib';

/* ------------------------------------------------------------------ shared */

interface TipState {
  x: number;
  y: number;
  title: string;
  rows: { label: string; value: string }[];
}

function Tooltip({ tip, width }: { tip: TipState | null; width: number }) {
  if (!tip) return null;
  // Keep the bubble inside the plot instead of letting it clip at the edges.
  const left = clamp(tip.x, 78, Math.max(78, width - 78));
  return (
    <div className="tooltip" style={{ left, top: tip.y }} role="presentation">
      <div className="tt-title">{tip.title}</div>
      {tip.rows.map((r) => (
        <div className="tt-row" key={r.label}>
          <span>{r.label}</span>
          <b>{r.value}</b>
        </div>
      ))}
    </div>
  );
}

/** Rect with only its data-end corners rounded, so the baseline stays crisp. */
function barPath(x: number, y: number, w: number, h: number, r: number, side: 'right' | 'left') {
  const radius = Math.max(0, Math.min(r, Math.abs(w)));
  if (Math.abs(w) < 0.6) return `M${x},${y} h${w} v${h} h${-w} Z`;
  return side === 'right'
    ? `M${x},${y} h${w - radius} a${radius},${radius} 0 0 1 ${radius},${radius} v${h - 2 * radius} a${radius},${radius} 0 0 1 ${-radius},${radius} h${-(w - radius)} Z`
    : `M${x},${y} h${-(w - radius)} a${radius},${radius} 0 0 0 ${-radius},${radius} v${h - 2 * radius} a${radius},${radius} 0 0 0 ${radius},${radius} h${w - radius} Z`;
}

/* --------------------------------------------------------------- bar chart */

export interface BarDatum {
  label: string;
  value: number;
  /** Shown under the value label in the tooltip. */
  rows?: { label: string; value: string }[];
  /** Overrides the diverging/sequential default. */
  color?: string;
  sublabel?: string;
}

interface BarChartProps {
  data: BarDatum[];
  format: (v: number) => string;
  /** Value that means "neutral". Set to 0.5 for win rate, 0 for differentials. */
  baseline?: number;
  domain?: [number, number];
  labelWidth?: number;
  rowHeight?: number;
  onSelect?: (label: string) => void;
  /** Colour job: diverging around the baseline, or one-hue magnitude. */
  scale?: 'diverging' | 'sequential';
  /** What the bar measures, named in the tooltip. */
  valueLabel?: string;
}

export function BarChart({
  data,
  format,
  baseline = 0,
  domain,
  labelWidth = 92,
  rowHeight = 30,
  onSelect,
  scale = 'diverging',
  valueLabel = 'Value',
}: BarChartProps) {
  const { ref, width } = useMeasure<HTMLDivElement>();
  const [tip, setTip] = useState<TipState | null>(null);

  const [lo, hi] = useMemo<[number, number]>(() => {
    if (domain) return domain;
    const values = data.map((d) => d.value);
    if (values.length === 0) return [baseline, baseline + 1];
    const max = Math.max(...values);
    const min = Math.min(...values);
    // The baseline is a real zero, so it always stays in frame. When every
    // value sits on one side of it the scale is one-sided and the bars use the
    // full width; only a genuinely two-sided set gets a symmetric scale.
    if (min >= baseline) return [baseline, max + (max - baseline) * 0.12 || baseline + 1];
    if (max <= baseline) return [min - (baseline - min) * 0.12 || baseline - 1, baseline];
    const reach = Math.max(max - baseline, baseline - min) * 1.12;
    return [baseline - reach, baseline + reach];
  }, [data, domain, baseline]);

  const padRight = 54;
  const plotW = Math.max(60, width - labelWidth - padRight);
  const height = data.length * rowHeight + 26;
  const x = (v: number) => labelWidth + ((clamp(v, lo, hi) - lo) / (hi - lo || 1)) * plotW;
  const zeroX = x(baseline);
  const barH = Math.min(14, rowHeight - 12);

  const maxAbs = Math.max(...data.map((d) => Math.abs(d.value - baseline)), 1e-6);

  return (
    <div className="chart dimmable" ref={ref}>
      <svg height={height} role="img" aria-label="Bar chart">
        {/* Baseline and its label */}
        <line className="axis-line" x1={zeroX} y1={6} x2={zeroX} y2={data.length * rowHeight + 6} />
        <text className="tick" x={zeroX} y={height - 4} textAnchor="middle">
          {format(baseline)}
        </text>

        {data.map((d, i) => {
          const y = i * rowHeight + 8;
          const end = x(d.value);
          const w = end - zeroX;
          const side = w >= 0 ? 'right' : 'left';
          const color =
            d.color ??
            (scale === 'diverging'
              ? winRateColor(baseline === 0.5 ? d.value : 0.5 + (d.value - baseline) / (maxAbs * 2))
              : sequentialBlue(Math.abs(d.value - baseline) / maxAbs));
          const isHot = tip?.title === d.label;
          return (
            <g key={d.label}>
              <rect
                className="row-hit"
                x={0}
                y={y - 4}
                width={Math.max(width, 1)}
                height={rowHeight}
                onMouseEnter={() =>
                  setTip({
                    x: end,
                    y: y + barH / 2,
                    title: d.label,
                    rows: [{ label: valueLabel, value: format(d.value) }, ...(d.rows ?? [])],
                  })
                }
                onMouseLeave={() => setTip(null)}
                onClick={() => onSelect?.(d.label)}
                style={{ cursor: onSelect ? 'pointer' : 'default' }}
              />
              <text
                className="tick"
                x={labelWidth - 12}
                y={y + barH / 2 + 4}
                textAnchor="end"
                style={{ fill: isHot ? 'var(--ink-1)' : undefined }}
              >
                {d.label}
              </text>
              <path
                className={`bar${isHot ? ' hovered' : ''}`}
                d={barPath(zeroX, y, w, barH, 4, side)}
                fill={color}
              />
              <text
                className="mark-label"
                x={side === 'right' ? end + 8 : end - 8}
                y={y + barH / 2 + 4}
                textAnchor={side === 'right' ? 'start' : 'end'}
              >
                {format(d.value)}
              </text>
            </g>
          );
        })}
      </svg>
      <Tooltip tip={tip} width={width} />
    </div>
  );
}

/* -------------------------------------------------------------- line chart */

export interface LineSeries {
  name: string;
  color: string;
  points: { x: string; y: number; label?: string }[];
}

interface LineChartProps {
  series: LineSeries[];
  yFormat: (v: number) => string;
  yDomain?: [number, number];
  height?: number;
  /** Optional dashed rule, e.g. the 50% line. */
  reference?: number;
  area?: boolean;
}

export function LineChart({
  series,
  yFormat,
  yDomain,
  height = 210,
  reference,
  area = false,
}: LineChartProps) {
  const { ref, width } = useMeasure<HTMLDivElement>();
  const [hover, setHover] = useState<number | null>(null);

  const count = Math.max(...series.map((s) => s.points.length), 0);
  const padL = 44;
  const padR = 14;
  const padT = 10;
  const padB = 26;
  const plotW = Math.max(40, width - padL - padR);
  const plotH = height - padT - padB;

  const [lo, hi] = useMemo<[number, number]>(() => {
    if (yDomain) return yDomain;
    const ys = series.flatMap((s) => s.points.map((p) => p.y));
    if (ys.length === 0) return [0, 1];
    const min = Math.min(...ys, reference ?? Infinity);
    const max = Math.max(...ys, reference ?? -Infinity);
    const pad = (max - min) * 0.18 || 0.5;
    return [min - pad, max + pad];
  }, [series, yDomain, reference]);

  const x = (i: number) => padL + (count <= 1 ? plotW / 2 : (i / (count - 1)) * plotW);
  const y = (v: number) => padT + (1 - (clamp(v, lo, hi) - lo) / (hi - lo || 1)) * plotH;

  const ticks = useMemo(() => {
    const steps = 4;
    return Array.from({ length: steps + 1 }, (_, i) => lo + ((hi - lo) * i) / steps);
  }, [lo, hi]);

  const onMove = (event: React.MouseEvent<SVGRectElement>) => {
    const rect = event.currentTarget.getBoundingClientRect();
    const rel = event.clientX - rect.left - padL;
    const idx = count <= 1 ? 0 : Math.round((rel / plotW) * (count - 1));
    setHover(clamp(idx, 0, count - 1));
  };

  const primary = series[0];
  const hoverPoint = hover !== null ? primary?.points[hover] : undefined;

  return (
    <div className="chart" ref={ref}>
      {series.length > 1 && (
        <div className="legend">
          {series.map((s) => (
            <span className="legend-item" key={s.name}>
              <span className="legend-swatch" style={{ background: s.color }} />
              {s.name}
            </span>
          ))}
        </div>
      )}
      <svg height={height} role="img" aria-label="Line chart">
        {ticks.map((t) => (
          <g key={t}>
            <line className="grid-line" x1={padL} y1={y(t)} x2={width - padR} y2={y(t)} />
            <text className="tick" x={padL - 8} y={y(t) + 3.5} textAnchor="end">
              {yFormat(t)}
            </text>
          </g>
        ))}
        {reference !== undefined && (
          <line
            className="crosshair"
            x1={padL}
            y1={y(reference)}
            x2={width - padR}
            y2={y(reference)}
          />
        )}

        {series.map((s) => {
          const path = s.points.map((p, i) => `${i === 0 ? 'M' : 'L'}${x(i)},${y(p.y)}`).join(' ');
          return (
            <g key={s.name}>
              {area && s.points.length > 1 && (
                <path
                  d={`${path} L${x(s.points.length - 1)},${padT + plotH} L${x(0)},${padT + plotH} Z`}
                  fill={mix('#131820', s.color, 0.18)}
                  opacity={0.85}
                />
              )}
              <path className="series-line" d={path} stroke={s.color} />
            </g>
          );
        })}

        {hover !== null && (
          <g>
            <line className="crosshair" x1={x(hover)} y1={padT} x2={x(hover)} y2={padT + plotH} />
            {series.map((s) => {
              const p = s.points[hover];
              return p ? (
                <circle
                  key={s.name}
                  className="dot"
                  cx={x(hover)}
                  cy={y(p.y)}
                  r={4.5}
                  fill={s.color}
                />
              ) : null;
            })}
          </g>
        )}

        {/* Hit area spans the whole plot so the crosshair never drops out. */}
        <rect
          x={0}
          y={0}
          width={Math.max(width, 1)}
          height={height}
          fill="transparent"
          onMouseMove={onMove}
          onMouseLeave={() => setHover(null)}
        />

        {count > 1 && (
          <>
            <text className="tick" x={padL} y={height - 6} textAnchor="start">
              {primary?.points[0]?.label ?? ''}
            </text>
            <text className="tick" x={width - padR} y={height - 6} textAnchor="end">
              {primary?.points[count - 1]?.label ?? ''}
            </text>
          </>
        )}
      </svg>
      {hover !== null && hoverPoint && (
        <Tooltip
          tip={{
            x: x(hover),
            y: y(hoverPoint.y),
            title: hoverPoint.label ?? hoverPoint.x,
            rows: series
              .filter((s) => s.points[hover])
              .map((s) => ({ label: s.name, value: yFormat(s.points[hover].y) })),
          }}
          width={width}
        />
      )}
    </div>
  );
}

/* ----------------------------------------------------------------- heatmap */

export interface HeatCell {
  value: number | null;
  /** Sample size behind the value — drives the tooltip and the empty state. */
  count: number;
  rows?: { label: string; value: string }[];
}

interface HeatmapProps {
  rowLabels: string[];
  colLabels: string[];
  cells: (HeatCell | null)[][];
  format: (v: number) => string;
  scale?: 'diverging' | 'sequential';
  rowWidth?: number;
  /**
   * Maps a raw value onto the 0-1 axis the diverging scale expects, so a
   * measure whose neutral point is not 0.5 (K/D sits at 1.0) still gets grey
   * at "even" and the two hues on either side.
   */
  normalize?: (v: number) => number;
}

export function Heatmap({
  rowLabels,
  colLabels,
  cells,
  format,
  scale = 'diverging',
  rowWidth = 96,
  normalize = (v) => v,
}: HeatmapProps) {
  const [tip, setTip] = useState<(TipState & { key: string }) | null>(null);
  const max = Math.max(
    ...cells.flat().map((c) => (c && c.value !== null ? c.value : 0)),
    1e-6,
  );

  return (
    <div className="chart">
      <div className="heatmap">
        <div
          className="heat-grid"
          style={{ gridTemplateColumns: `${rowWidth}px repeat(${colLabels.length}, minmax(44px, 1fr))` }}
        >
          <div />
          {colLabels.map((c) => (
            <div className="heat-head col" key={c}>
              {c}
            </div>
          ))}
          {rowLabels.map((r, ri) => (
            <div style={{ display: 'contents' }} key={r}>
              <div className="heat-head">{r}</div>
              {colLabels.map((c, ci) => {
                const cell = cells[ri]?.[ci];
                const key = `${r}-${c}`;
                if (!cell || cell.value === null) {
                  return (
                    <div className="heat-cell blank" key={key} title={`${r} — no games on ${c}`}>
                      ·
                    </div>
                  );
                }
                // Capture the value after the null guard: TypeScript drops
                // property narrowing inside the event callbacks below.
                const value = cell.value;
                const bg =
                  scale === 'diverging'
                    ? winRateColor(normalize(value))
                    : sequentialBlue(value / max);
                // Confidence is carried by opacity; the count is in the tooltip.
                const strength = clamp(0.45 + cell.count / 12, 0.45, 1);
                return (
                  <div
                    className="heat-cell"
                    key={key}
                    style={{ background: bg, opacity: strength, color: '#fff' }}
                    onMouseEnter={(e) => {
                      const host = e.currentTarget.closest('.chart') as HTMLElement | null;
                      const box = e.currentTarget.getBoundingClientRect();
                      const parent = host?.getBoundingClientRect();
                      setTip({
                        key,
                        x: box.left - (parent?.left ?? 0) + box.width / 2,
                        y: box.top - (parent?.top ?? 0),
                        title: `${r} · ${c}`,
                        rows: [
                          { label: 'Value', value: format(value) },
                          { label: 'Games', value: String(cell.count) },
                          ...(cell.rows ?? []),
                        ],
                      });
                    }}
                    onMouseLeave={() => setTip(null)}
                  >
                    {format(value)}
                  </div>
                );
              })}
            </div>
          ))}
        </div>
      </div>
      <Tooltip tip={tip} width={9999} />
    </div>
  );
}

/* --------------------------------------------------------------- sparkline */

export function Sparkline({
  values,
  color = '#3987e5',
  height = 34,
  reference,
}: {
  values: number[];
  color?: string;
  height?: number;
  reference?: number;
}) {
  const { ref, width } = useMeasure<HTMLDivElement>();
  if (values.length < 2) return <div ref={ref} style={{ height }} />;

  const lo = Math.min(...values, reference ?? Infinity);
  const hi = Math.max(...values, reference ?? -Infinity);
  const x = (i: number) => (i / (values.length - 1)) * Math.max(width - 4, 10) + 2;
  const y = (v: number) => height - 3 - ((v - lo) / (hi - lo || 1)) * (height - 6);
  const path = values.map((v, i) => `${i === 0 ? 'M' : 'L'}${x(i)},${y(v)}`).join(' ');

  return (
    <div className="chart" ref={ref}>
      <svg height={height} aria-hidden="true">
        {reference !== undefined && (
          <line className="crosshair" x1={0} y1={y(reference)} x2={width} y2={y(reference)} />
        )}
        <path
          d={`${path} L${x(values.length - 1)},${height} L${x(0)},${height} Z`}
          fill={mix('#131820', color, 0.16)}
        />
        <path className="series-line" d={path} stroke={color} />
        <circle cx={x(values.length - 1)} cy={y(values[values.length - 1])} r={3} fill={color} />
      </svg>
    </div>
  );
}

/* ------------------------------------------------------------- misc blocks */

export function FormStrip({
  entries,
}: {
  entries: { result: 'win' | 'loss' | 'draw'; title: string }[];
}) {
  return (
    <div className="form-strip">
      {entries.map((e, i) => (
        <div className={`form-chip ${e.result}`} key={i} title={e.title}>
          {e.result === 'win' ? 'W' : e.result === 'loss' ? 'L' : 'D'}
        </div>
      ))}
    </div>
  );
}

/** Horizontal stacked bar showing how many of each role a comp runs. */
export function RoleSpread({ spread }: { spread: Record<string, number> }) {
  const order = ['Duelist', 'Initiator', 'Controller', 'Sentinel'];
  const total = order.reduce((sum, r) => sum + (spread[r] ?? 0), 0) || 1;
  const colors: Record<string, string> = {
    Duelist: '#d95926',
    Initiator: '#3987e5',
    Controller: '#d55181',
    Sentinel: '#199e70',
  };
  return (
    <div>
      <div style={{ display: 'flex', gap: 2, height: 8, borderRadius: 4, overflow: 'hidden' }}>
        {order.map((r) =>
          (spread[r] ?? 0) > 0 ? (
            <div
              key={r}
              style={{ width: `${((spread[r] ?? 0) / total) * 100}%`, background: colors[r] }}
              title={`${spread[r]} × ${r}`}
            />
          ) : null,
        )}
      </div>
      <div className="legend" style={{ paddingTop: 10, paddingBottom: 0 }}>
        {order.map((r) => (
          <span className="legend-item" key={r} style={{ opacity: (spread[r] ?? 0) > 0 ? 1 : 0.4 }}>
            <span className="legend-swatch" style={{ background: colors[r] }} />
            {spread[r] ?? 0} {r}
          </span>
        ))}
      </div>
    </div>
  );
}

export function ChartFrame({
  title,
  subtitle,
  actions,
  children,
}: {
  title: string;
  subtitle?: string;
  actions?: ReactNode;
  children: ReactNode;
}) {
  return (
    <section className="card">
      <div className="card-head">
        <div className="grow">
          <div className="card-title">{title}</div>
          {subtitle && <div className="card-sub">{subtitle}</div>}
        </div>
        {actions}
      </div>
      <div className="card-body">{children}</div>
    </section>
  );
}
