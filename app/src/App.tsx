import { useEffect } from 'react';
import { Routes, Route, Navigate } from 'react-router-dom';
import TopBar from './components/TopBar';
import ProgressModal from './components/ProgressModal';
import AiStatusBar from './components/AiStatusBar';
import Overview from './pages/Overview';
import AssetGraph from './pages/AssetGraph';
import CodeGraph from './pages/CodeGraph';
import SuspectedRefs from './pages/SuspectedRefs';
import Hardcode from './pages/Hardcode';
import Settings from './pages/Settings';
import { useAppStore } from './store';

export default function App() {
  const loadSettings = useAppStore(s => s.loadSettings);
  useEffect(() => { loadSettings(); }, [loadSettings]);
  return (
    <div className="app-root">
      <TopBar />
      <ProgressModal />
      <AiStatusBar />
      <Routes>
        <Route path="/" element={<Navigate to="/overview" replace />} />
        <Route path="/overview" element={<Overview />} />
        <Route path="/asset" element={<AssetGraph />} />
        <Route path="/code" element={<CodeGraph />} />
        <Route path="/suspected" element={<SuspectedRefs />} />
        <Route path="/hardcode" element={<Hardcode />} />
        <Route path="/settings" element={<Settings />} />
      </Routes>
    </div>
  );
}
