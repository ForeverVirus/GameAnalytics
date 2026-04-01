import { useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import { useAppStore } from '../store';
import { useTranslation } from 'react-i18next';

export default function ArtDashboard() {
  const navigate = useNavigate();
  const { t } = useTranslation();
  const stats = useAppStore((s) => s.stats);
  const orphans = useAppStore((s) => s.orphans);
  const duplicates = useAppStore((s) => s.duplicates);
  const assetMetrics = useAppStore((s) => s.assetMetrics);
  const loadOrphans = useAppStore((s) => s.loadOrphans);
  const loadDuplicates = useAppStore((s) => s.loadDuplicates);
  const loadAssetMetrics = useAppStore((s) => s.loadAssetMetrics);

  useEffect(() => {
    loadOrphans();
    loadDuplicates();
    loadAssetMetrics();
  }, []);

  const assetOrphans = orphans.filter((o) => o.node_type === 'Asset');
  const poorAssets = assetMetrics.filter((m) => m.performance_rating === 'poor');
  const fairAssets = assetMetrics.filter((m) => m.performance_rating === 'fair');
  const totalMemory = assetMetrics.reduce((sum, m) => sum + (m.estimated_memory_bytes ?? 0), 0);

  // Optimization score: 100 - penalties for poor/fair assets
  const totalAnalyzed = assetMetrics.length || 1;
  const poorPenalty = Math.round((poorAssets.length / totalAnalyzed) * 60);
  const fairPenalty = Math.round((fairAssets.length / totalAnalyzed) * 20);
  const orphanPenalty = Math.min(assetOrphans.length, 20);
  const optScore = Math.max(0, 100 - poorPenalty - fairPenalty - orphanPenalty);
  const scoreClass = optScore >= 80 ? 'score-good' : optScore >= 50 ? 'score-fair' : 'score-poor';

  return (
    <div className="dashboard-page">
      <h1 className="page-title">{t('art.dashboard.title', '美术工作台')}</h1>

      {/* Optimization Score */}
      <div className="health-score-card">
        <div className={`health-score-value ${scoreClass}`}>{optScore}</div>
        <div className="health-score-label">
          <strong>{t('art.dashboard.optScore', '资源优化度')}</strong>
          {optScore >= 80 ? t('art.dashboard.optGood', '资源状态良好') : optScore >= 50 ? t('art.dashboard.optFair', '存在优化空间') : t('art.dashboard.optPoor', '需要重点优化')}
        </div>
        <div className="health-score-breakdown">
          <span>⚠️ {poorAssets.length} {t('art.dashboard.poorCount', '差')}</span>
          <span>⚡ {fairAssets.length} {t('art.dashboard.fairCount', '一般')}</span>
          <span>📁 {assetOrphans.length} {t('art.dashboard.orphanCount', '孤立')}</span>
        </div>
      </div>

      {/* Stats Overview */}
      <div className="stats-grid">
        <div className="stat-card">
          <div className="stat-value">{stats?.asset_count ?? '-'}</div>
          <div className="stat-label">{t('art.dashboard.totalAssets', '总资源数')}</div>
        </div>
        <div className="stat-card">
          <div className="stat-value">{assetMetrics.length}</div>
          <div className="stat-label">{t('art.dashboard.analyzed', '已分析资源')}</div>
        </div>
        <div className="stat-card">
          <div className="stat-value">{formatSize(totalMemory)}</div>
          <div className="stat-label">{t('art.dashboard.estMemory', '估计显存占用')}</div>
        </div>
        <div className="stat-card">
          <div className="stat-value">{poorAssets.length + fairAssets.length}</div>
          <div className="stat-label">{t('art.dashboard.needOptimize', '需优化资源')}</div>
        </div>
      </div>

      {/* Insight Cards */}
      <div className="insight-grid">
        <div className="insight-card danger" onClick={() => navigate('/art/analysis')}>
          <h3>{t('art.dashboard.poorTitle', '性能问题资源')}</h3>
          <div className="insight-value">{poorAssets.length}</div>
          <div className="insight-detail">{t('art.dashboard.poorDesc', '分辨率过高或内存占用过大')}</div>
          <span className="insight-action">{t('common.viewDetails', '查看详情')} →</span>
        </div>

        <div className="insight-card warning" onClick={() => navigate('/art/analysis')}>
          <h3>{t('art.dashboard.fairTitle', '可优化资源')}</h3>
          <div className="insight-value">{fairAssets.length}</div>
          <div className="insight-detail">{t('art.dashboard.fairDesc', '存在优化空间')}</div>
          <span className="insight-action">{t('common.viewDetails', '查看详情')} →</span>
        </div>

        <div className="insight-card warning" onClick={() => navigate('/art/redundancy')}>
          <h3>{t('art.dashboard.orphanTitle', '孤立资源')}</h3>
          <div className="insight-value">{assetOrphans.length}</div>
          <div className="insight-detail">{t('art.dashboard.orphanDesc', '未被引用的资源文件')}</div>
          <span className="insight-action">{t('common.viewDetails', '查看详情')} →</span>
        </div>

        <div className="insight-card accent" onClick={() => navigate('/art/analysis')}>
          <h3>{t('art.dashboard.aiTitle', 'AI 优化建议')}</h3>
          <div className="insight-value">🤖</div>
          <div className="insight-detail">{t('art.dashboard.aiDesc', '智能分析并给出优化方案')}</div>
          <span className="insight-action">{t('art.dashboard.getAdvice', '获取建议')} →</span>
        </div>
      </div>

      {/* Quick Links */}
      <div className="quick-links">
        <h2>{t('art.dashboard.tools', '查看工具')}</h2>
        <div className="link-grid">
          <div className="link-card" onClick={() => navigate('/asset')}>
            <span className="link-icon">📦</span>
            <span>{t('nav.asset', '资源图谱')}</span>
          </div>
          <div className="link-card" onClick={() => navigate('/suspected')}>
            <span className="link-icon">🔗</span>
            <span>{t('nav.suspected', '疑似引用')}</span>
          </div>
        </div>
      </div>
    </div>
  );
}

function formatSize(bytes: number): string {
  if (bytes === 0) return '0 B';
  const units = ['B', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(1024));
  return (bytes / Math.pow(1024, i)).toFixed(1) + ' ' + units[i];
}
