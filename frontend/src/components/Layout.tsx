import { NavLink, useLocation } from 'react-router-dom';
import type { ReactNode } from 'react';

const icon = (path: ReactNode) => (
  <svg width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7"
       strokeLinecap="round" strokeLinejoin="round">
    {path}
  </svg>
);

export const Icons = {
  dashboard: icon(<><rect x="3" y="3" width="7" height="8" rx="1.5" /><rect x="14" y="3" width="7" height="5" rx="1.5" /><rect x="14" y="11" width="7" height="10" rx="1.5" /><rect x="3" y="14" width="7" height="7" rx="1.5" /></>),
  roster: icon(<><circle cx="9" cy="8" r="3.2" /><path d="M3 20c0-3.3 2.7-5.4 6-5.4s6 2.1 6 5.4" /><path d="M16 4.6a3.2 3.2 0 0 1 0 6.3" /><path d="M18 14.9c2 .7 3 2.4 3 5.1" /></>),
  maps: icon(<><path d="M9 4.5 3 7v13l6-2.5 6 2.5 6-2.5V4L15 6.5z" /><path d="M9 4.5v13M15 6.5v13" /></>),
  agents: icon(<><circle cx="12" cy="12" r="8.5" /><circle cx="12" cy="12" r="3" /><path d="M12 1.8v3.4M12 18.8v3.4M1.8 12h3.4M18.8 12h3.4" /></>),
  matches: icon(<><path d="M4 6h16M4 12h16M4 18h10" /><circle cx="19.5" cy="18" r="1.6" /></>),
  import: icon(<><path d="M12 3v12" /><path d="m7.5 10.5 4.5 4.5 4.5-4.5" /><path d="M4 17v2.5A1.5 1.5 0 0 0 5.5 21h13a1.5 1.5 0 0 0 1.5-1.5V17" /></>),
  builder: icon(<><path d="M12 3v3M12 18v3M3 12h3M18 12h3" /><path d="m6.5 6.5 2.2 2.2M15.3 15.3l2.2 2.2M17.5 6.5l-2.2 2.2M8.7 15.3l-2.2 2.2" /><circle cx="12" cy="12" r="3.4" /></>),
};

const NAV = [
  { to: '/', label: 'Dashboard', icon: Icons.dashboard, end: true },
  { to: '/builder', label: 'Team Builder', icon: Icons.builder, end: false },
  { to: '/roster', label: 'Roster', icon: Icons.roster, end: false },
  { to: '/maps', label: 'Maps', icon: Icons.maps, end: false },
  { to: '/agents', label: 'Agents', icon: Icons.agents, end: false },
  { to: '/matches', label: 'Matches', icon: Icons.matches, end: false },
  { to: '/import', label: 'Import', icon: Icons.import, end: false },
];

export function Sidebar() {
  const { pathname } = useLocation();
  return (
    <aside className="sidebar">
      <div className="brand">
        <div className="brand-mark">
          <svg width="18" height="18" viewBox="0 0 32 32" aria-hidden="true">
            <path d="M6 7l10 12L26 7v6L16 25 6 13z" fill="#0b0e14" />
          </svg>
        </div>
        <div className="brand-text">
          <div className="brand-name">valops</div>
          <div className="brand-sub">five-stack</div>
        </div>
      </div>

      <div className="nav-label">Command</div>
      {NAV.map((item) => (
        <NavLink
          key={item.to}
          to={item.to}
          end={item.end}
          className={({ isActive }) =>
            `nav-item${isActive || (item.to !== '/' && pathname.startsWith(item.to)) ? ' active' : ''}`
          }
        >
          {item.icon}
          <span>{item.label}</span>
        </NavLink>
      ))}

      <div className="sidebar-foot">
        <span>Self-hosted · your data stays in your container</span>
      </div>
    </aside>
  );
}

export function TopBar({
  title,
  subtitle,
  children,
}: {
  title: string;
  subtitle?: string;
  children?: ReactNode;
}) {
  return (
    <header className="topbar">
      <div>
        <h1>{title}</h1>
        {subtitle && <div className="sub">{subtitle}</div>}
      </div>
      <div className="topbar-actions">{children}</div>
    </header>
  );
}

export function WindowPicker({
  days,
  onChange,
}: {
  days: number | null;
  onChange: (days: number | null) => void;
}) {
  const options: { label: string; value: number | null }[] = [
    { label: 'All time', value: null },
    { label: '90 days', value: 90 },
    { label: '30 days', value: 30 },
  ];
  return (
    <div className="seg" role="group" aria-label="Time window">
      {options.map((o) => (
        <button
          key={o.label}
          aria-pressed={days === o.value}
          onClick={() => onChange(o.value)}
          type="button"
        >
          {o.label}
        </button>
      ))}
    </div>
  );
}
