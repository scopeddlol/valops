/** Small shared building blocks. */

import { useEffect, type ReactNode } from 'react';
import type { Result } from '../api';
import { initials, mix, seriesColor } from '../lib';

export function Avatar({
  name,
  index,
  size = 'md',
}: {
  name: string;
  index: number;
  size?: 'md' | 'sm';
}) {
  const color = seriesColor(index);
  return (
    <div
      className={`avatar${size === 'sm' ? ' sm' : ''}`}
      style={{
        background: `linear-gradient(145deg, ${color}, ${mix(color, '#0b0e14', 0.42)})`,
        boxShadow: `0 6px 16px -8px ${color}`,
      }}
      aria-hidden="true"
    >
      {initials(name)}
    </div>
  );
}

export function ResultPill({ result, score }: { result: Result; score?: string }) {
  const letter = result === 'win' ? 'W' : result === 'loss' ? 'L' : 'D';
  return (
    <span className={`pill ${result}`}>
      {letter}
      {score ? <span className="num">{score}</span> : null}
    </span>
  );
}

export function RoleTag({ role }: { role: string }) {
  return (
    <span className="role-tag">
      <span className={`role-dot role-${role}`} />
      {role}
    </span>
  );
}

export function StatTile({
  label,
  value,
  unit,
  foot,
  accent,
  children,
}: {
  label: string;
  value: ReactNode;
  unit?: string;
  foot?: ReactNode;
  accent?: string;
  children?: ReactNode;
}) {
  return (
    <div className="stat" style={accent ? ({ '--tile-accent': accent } as React.CSSProperties) : undefined}>
      <div className="stat-label">{label}</div>
      <div className="stat-value">
        {value}
        {unit && <span className="unit">{unit}</span>}
      </div>
      {foot && <div className="stat-foot">{foot}</div>}
      {children}
    </div>
  );
}

export function Meter({ value, color = 'var(--s1)' }: { value: number; color?: string }) {
  return (
    <div className="meter">
      <span style={{ width: `${Math.max(2, Math.min(100, value * 100))}%`, background: color }} />
    </div>
  );
}

export function Empty({
  title,
  message,
  action,
}: {
  title: string;
  message: string;
  action?: ReactNode;
}) {
  return (
    <div className="empty-state">
      <svg width="34" height="34" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.4">
        <path d="M3 3v18h18" strokeLinecap="round" />
        <path d="M7 15l4-5 3 3 4-6" strokeLinecap="round" strokeLinejoin="round" />
      </svg>
      <h3>{title}</h3>
      <p>{message}</p>
      {action}
    </div>
  );
}

export function Loading({ rows = 3, height = 96 }: { rows?: number; height?: number }) {
  return (
    <div className="grid" style={{ gap: 14 }}>
      {Array.from({ length: rows }, (_, i) => (
        <div className="skeleton" key={i} style={{ height }} />
      ))}
    </div>
  );
}

export function ErrorBanner({ message, onRetry }: { message: string; onRetry?: () => void }) {
  return (
    <div className="banner">
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
        <circle cx="12" cy="12" r="9" />
        <path d="M12 7v6M12 16.5v.5" strokeLinecap="round" />
      </svg>
      <span>{message}</span>
      {onRetry && (
        <button className="btn sm ghost" style={{ marginLeft: 'auto' }} onClick={onRetry}>
          Retry
        </button>
      )}
    </div>
  );
}

export function Modal({
  title,
  onClose,
  children,
  footer,
}: {
  title: string;
  onClose: () => void;
  children: ReactNode;
  footer?: ReactNode;
}) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose();
    };
    document.addEventListener('keydown', onKey);
    document.body.style.overflow = 'hidden';
    return () => {
      document.removeEventListener('keydown', onKey);
      document.body.style.overflow = '';
    };
  }, [onClose]);

  return (
    <div
      className="modal-backdrop"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div className="modal" role="dialog" aria-modal="true" aria-label={title}>
        <div className="modal-head">
          <h2 style={{ fontSize: 17 }}>{title}</h2>
          <button className="btn sm ghost" style={{ marginLeft: 'auto' }} onClick={onClose} aria-label="Close">
            ✕
          </button>
        </div>
        <div className="modal-body">{children}</div>
        {footer && <div className="modal-foot">{footer}</div>}
      </div>
    </div>
  );
}

/** Inline bar behind a table value, so a column of numbers reads as a shape. */
export function CellBar({ value, color }: { value: number; color?: string }) {
  return (
    <span className="cell-bar" aria-hidden="true">
      <span style={{ width: `${Math.max(2, Math.min(100, value * 100))}%`, background: color }} />
    </span>
  );
}
