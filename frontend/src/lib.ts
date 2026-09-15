/** Formatting and colour helpers shared by every view. */

import type { Result, Role } from './api';

/* ------------------------------------------------------------------ format */

export const pct = (v: number | null | undefined, digits = 0): string =>
  v === null || v === undefined ? '—' : `${(v * 100).toFixed(digits)}%`;

export const num = (v: number | null | undefined, digits = 2): string =>
  v === null || v === undefined ? '—' : v.toFixed(digits);

export const signed = (v: number, digits = 0): string =>
  `${v > 0 ? '+' : ''}${v.toFixed(digits)}`;

export function shortDate(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso.slice(0, 10);
  return d.toLocaleDateString(undefined, { month: 'short', day: 'numeric' });
}

export function fullDate(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return d.toLocaleDateString(undefined, { year: 'numeric', month: 'short', day: 'numeric' });
}

export function relativeDate(iso: string | null): string {
  if (!iso) return 'never';
  const then = new Date(iso).getTime();
  if (Number.isNaN(then)) return iso;
  const days = Math.round((Date.now() - then) / 86_400_000);
  if (days <= 0) return 'today';
  if (days === 1) return 'yesterday';
  if (days < 30) return `${days}d ago`;
  if (days < 365) return `${Math.round(days / 30)}mo ago`;
  return `${Math.round(days / 365)}y ago`;
}

export function initials(name: string): string {
  const parts = name.trim().split(/[\s_\-.]+/).filter(Boolean);
  if (parts.length === 0) return '?';
  if (parts.length === 1) return parts[0].slice(0, 2).toUpperCase();
  return (parts[0][0] + parts[1][0]).toUpperCase();
}

export function streakLabel(streak: number): string {
  if (streak === 0) return 'No streak';
  return `${Math.abs(streak)} ${streak > 0 ? 'win' : 'loss'}${Math.abs(streak) === 1 ? '' : 'es'} in a row`
    .replace('losses in a row', 'losses in a row');
}

/* ------------------------------------------------------------------ colour */

/**
 * Categorical slots in fixed order — a player keeps their colour no matter how
 * the list is filtered or sorted. Past eight entities we fold to neutral rather
 * than inventing a ninth hue.
 */
export const SERIES = [
  '#3987e5',
  '#d95926',
  '#199e70',
  '#c98500',
  '#d55181',
  '#9085e9',
  '#008300',
  '#e66767',
] as const;

export const seriesColor = (index: number): string =>
  index >= 0 && index < SERIES.length ? SERIES[index] : '#66748a';

const hexToRgb = (hex: string): [number, number, number] => {
  const h = hex.replace('#', '');
  return [
    parseInt(h.slice(0, 2), 16),
    parseInt(h.slice(2, 4), 16),
    parseInt(h.slice(4, 6), 16),
  ];
};

const toLinear = (c: number) => {
  const s = c / 255;
  return s <= 0.04045 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
};

const toSrgb = (c: number) => {
  const s = c <= 0.0031308 ? c * 12.92 : 1.055 * c ** (1 / 2.4) - 0.055;
  return Math.round(Math.min(1, Math.max(0, s)) * 255);
};

/** Blend two hex colours in linear light, which keeps mid-tones from going muddy. */
export function mix(a: string, b: string, t: number): string {
  const [ar, ag, ab] = hexToRgb(a).map(toLinear);
  const [br, bg, bb] = hexToRgb(b).map(toLinear);
  const k = Math.min(1, Math.max(0, t));
  const r = toSrgb(ar + (br - ar) * k);
  const g = toSrgb(ag + (bg - ag) * k);
  const bl = toSrgb(ab + (bb - ab) * k);
  return `#${[r, g, bl].map((v) => v.toString(16).padStart(2, '0')).join('')}`;
}

const DIV_POS = '#3987e5';
const DIV_NEG = '#e66767';
const DIV_MID = '#3a4350';

/**
 * Win rate is a polarity, not a magnitude: 50% is the neutral midpoint and the
 * two arms get opposite hues. `spread` is the rate distance that saturates.
 */
export function winRateColor(rate: number, spread = 0.28): string {
  const delta = rate - 0.5;
  const t = Math.min(1, Math.abs(delta) / spread);
  return mix(DIV_MID, delta >= 0 ? DIV_POS : DIV_NEG, t);
}

/** One-hue sequential ramp for magnitude on a dark surface: near-zero recedes. */
export function sequentialBlue(t: number): string {
  const k = Math.min(1, Math.max(0, t));
  return mix('#153a63', '#86b6ef', k);
}

export const resultClass = (r: Result): string => r;

export const ROLE_ORDER: Role[] = ['Duelist', 'Initiator', 'Controller', 'Sentinel', 'Flex'];

export const roleColor = (role: string): string =>
  ({
    Duelist: '#d95926',
    Initiator: '#3987e5',
    Controller: '#d55181',
    Sentinel: '#199e70',
  })[role] ?? '#66748a';

/** Clamp helper used by the chart scales. */
export const clamp = (v: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, v));

/** Local `YYYY-MM-DDTHH:mm` string for datetime-local inputs. */
export function localDateTimeValue(d = new Date()): string {
  const pad = (n: number) => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T${pad(d.getHours())}:${pad(d.getMinutes())}`;
}
