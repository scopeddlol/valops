import { createContext, useContext, useMemo, useState, type ReactNode } from 'react';
import { Route, Routes } from 'react-router-dom';

import { Sidebar } from './components/Layout';
import Dashboard from './pages/Dashboard';
import Builder from './pages/Builder';
import Roster from './pages/Roster';
import Maps from './pages/Maps';
import Agents from './pages/Agents';
import Matches from './pages/Matches';
import ImportPage from './pages/Import';

interface FilterContextValue {
  days: number | null;
  setDays: (days: number | null) => void;
  /** Bumped whenever data is written, so every open view refetches. */
  version: number;
  invalidate: () => void;
}

const FilterContext = createContext<FilterContextValue>({
  days: null,
  setDays: () => {},
  version: 0,
  invalidate: () => {},
});

export const useFilters = () => useContext(FilterContext);

function FilterProvider({ children }: { children: ReactNode }) {
  const [days, setDays] = useState<number | null>(null);
  const [version, setVersion] = useState(0);
  const value = useMemo<FilterContextValue>(
    () => ({ days, setDays, version, invalidate: () => setVersion((v) => v + 1) }),
    [days, version],
  );
  return <FilterContext.Provider value={value}>{children}</FilterContext.Provider>;
}

export default function App() {
  return (
    <FilterProvider>
      <div className="shell">
        <Sidebar />
        <main className="main">
          <Routes>
            <Route path="/" element={<Dashboard />} />
            <Route path="/builder" element={<Builder />} />
            <Route path="/roster" element={<Roster />} />
            <Route path="/maps" element={<Maps />} />
            <Route path="/agents" element={<Agents />} />
            <Route path="/matches" element={<Matches />} />
            <Route path="/import" element={<ImportPage />} />
            <Route path="*" element={<Dashboard />} />
          </Routes>
        </main>
      </div>
    </FilterProvider>
  );
}
