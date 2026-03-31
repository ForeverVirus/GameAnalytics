import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useNavigate } from 'react-router-dom';
import { open } from '@tauri-apps/plugin-dialog';
import { useAppStore } from '../store';

export default function Overview() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const { project, stats, loading, startFullPipeline, exportAnalysis } = useAppStore();
  const [exportPath, setExportPath] = useState<string | null>(null);

  const handleSelectProject = async () => {
    const selected = await open({ directory: true, multiple: false });
    if (selected) {
      await startFullPipeline(selected as string);
    }
  };

  const handleReanalyze = async () => {
    if (project?.path) {
      await startFullPipeline(project.path, true);
    }
  };

  const handleExport = async () => {
    const path = await exportAnalysis();
    if (path) {
      setExportPath(path);
      setTimeout(() => setExportPath(null), 5000);
    }
  };

  if (!project) {
    return (
      <div className="page-main">
        <div className="empty-state">
          <div className="empty-icon">📁</div>
          <div className="empty-title">{t('overview.noProject')}</div>
          <div className="empty-desc">{t('overview.selectProjectHint')}</div>
          <button className="btn btn-primary" onClick={handleSelectProject}>
            📂 {t('overview.selectProject')}
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className="page-main">
      {/* Project bar */}
      <div className="project-bar">
        <div className="icon">📁</div>
        <div className="info">
          <div className="label">{t('overview.currentProject')}</div>
          <div className="path">
            {project.path.split(/[\\/]/).pop()}
            <span className="engine">{project.engine}</span>
          </div>
          <div className="meta">{project.path}</div>
        </div>
        <button className="btn btn-ghost" onClick={handleSelectProject}>
          📂 {t('overview.switchProject')}
        </button>
        <button className="btn btn-ghost" onClick={handleReanalyze} disabled={loading}>
          🔄 {t('overview.reanalyze')}
        </button>
        <button className="btn btn-primary" onClick={handleExport}>
          📤 {t('overview.exportReport')}
        </button>
      </div>

      {/* Export success toast */}
      {exportPath && (
        <div className="export-toast">
          ✅ {t('progress.exportDone')}: {exportPath}
        </div>
      )}

      {/* Stats grid */}
      <div className="stats-grid">
        <div className="stat-card">
          <div className="stat-icon">📦</div>
          <div className="stat-value">{stats?.asset_count ?? 0}</div>
          <div className="stat-label">{t('overview.assetFiles')}</div>
        </div>
        <div className="stat-card">
          <div className="stat-icon">📜</div>
          <div className="stat-value">{stats?.script_count ?? 0}</div>
          <div className="stat-label">{t('overview.scriptFiles')}</div>
        </div>
        <div className="stat-card">
          <div className="stat-icon">🔍</div>
          <div className="stat-value">{stats?.suspected_count ?? 0}</div>
          <div className="stat-label">{t('overview.suspectedRefs')}</div>
          <div className="stat-trend warn">⚠ {t('overview.needConfirm')}</div>
        </div>
        <div className="stat-card">
          <div className="stat-icon">⚡</div>
          <div className="stat-value">{stats?.hardcode_count ?? 0}</div>
          <div className="stat-label">{t('overview.hardcodeDetect')}</div>
          <div className="stat-trend warn">⚠ {t('overview.suggestFix')}</div>
        </div>
      </div>

      {/* Entry cards */}
      <div className="entry-grid">
        <div className="entry-card suspected" onClick={() => navigate('/suspected')}>
          <div className="card-header">
            <div className="card-icon">🔍</div>
            <div>
              <div className="card-title">{t('overview.suspectedTitle')}</div>
              <div className="card-subtitle">{t('overview.suspectedSubtitle')}</div>
            </div>
          </div>
          <div className="card-body">{t('overview.suspectedDesc')}</div>
          <div className="card-stats">
            <div className="card-stat">
              <span className="num">{stats?.suspected_count ?? 0}</span>
              <span className="unit">{t('overview.pending')}</span>
            </div>
          </div>
        </div>

        <div className="entry-card hardcode" onClick={() => navigate('/hardcode')}>
          <div className="card-header">
            <div className="card-icon">⚡</div>
            <div>
              <div className="card-title">{t('overview.hardcodeTitle')}</div>
              <div className="card-subtitle">{t('overview.hardcodeSubtitle')}</div>
            </div>
          </div>
          <div className="card-body">{t('overview.hardcodeDesc')}</div>
          <div className="card-stats">
            <div className="card-stat">
              <span className="num">{stats?.hardcode_count ?? 0}</span>
              <span className="unit">{t('overview.found')}</span>
            </div>
          </div>
        </div>
      </div>

      {/* Quick actions */}
      <div className="section-title">{t('overview.quickActions')}</div>
      <div className="quick-actions">
        <div className="quick-action" onClick={() => navigate('/asset')}>
          <div className="qa-icon">📊</div>
          <div>
            <div className="qa-title">{t('overview.viewAssetGraph')}</div>
            <div className="qa-desc">{t('overview.viewAssetGraphDesc')}</div>
          </div>
        </div>
        <div className="quick-action" onClick={() => navigate('/code')}>
          <div className="qa-icon">🔗</div>
          <div>
            <div className="qa-title">{t('overview.viewCodeGraph')}</div>
            <div className="qa-desc">{t('overview.viewCodeGraphDesc')}</div>
          </div>
        </div>
        <div className="quick-action" onClick={() => navigate('/settings')}>
          <div className="qa-icon">🤖</div>
          <div>
            <div className="qa-title">{t('overview.aiSettings')}</div>
            <div className="qa-desc">{t('overview.aiSettingsDesc')}</div>
          </div>
        </div>
      </div>
    </div>
  );
}
