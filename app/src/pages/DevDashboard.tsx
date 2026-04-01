import { useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import { useAppStore } from '../store';
import { useTranslation } from 'react-i18next';

export default function DevDashboard() {
  const navigate = useNavigate();
  const { t } = useTranslation();
  const stats = useAppStore((s) => s.stats);
  const orphans = useAppStore((s) => s.orphans);
  const duplicates = useAppStore((s) => s.duplicates);
  const hotspots = useAppStore((s) => s.hotspots);
  const loadOrphans = useAppStore((s) => s.loadOrphans);
  const loadDuplicates = useAppStore((s) => s.loadDuplicates);
  const loadHotspots = useAppStore((s) => s.loadHotspots);

  useEffect(() => {
    loadOrphans();
    loadDuplicates();
    loadHotspots();
  }, []);

  const totalOrphanSize = orphans.reduce((sum, o) => sum + o.file_size_bytes, 0);
  const totalDupSize = duplicates.reduce((sum, g) => sum + g.total_size, 0);
  const highRiskHotspots = hotspots.filter((h) => h.risk_level === 'high').length;

  // Simple health score: 100 - penalties
  const orphanPenalty = Math.min(orphans.length * 2, 20);
  const dupPenalty = Math.min(duplicates.length * 3, 20);
  const hotspotPenalty = Math.min(highRiskHotspots * 5, 20);
  const healthScore = Math.max(0, 100 - orphanPenalty - dupPenalty - hotspotPenalty);
  const scoreClass = healthScore >= 80 ? 'score-good' : healthScore >= 50 ? 'score-fair' : 'score-poor';

  return (
    <div className="dashboard-page">
      <h1 className="page-title">{t('dev.dashboard.title', '程序员工作台')}</h1>

      {/* Health Score */}
      <div className="health-score-card">
        <div className={`health-score-value ${scoreClass}`}>{healthScore}</div>
        <div className="health-score-label">
          <strong>{t('dev.dashboard.healthScore', '代码健康度')}</strong>
          {healthScore >= 80 ? t('dev.dashboard.healthGood', '项目状态良好') : healthScore >= 50 ? t('dev.dashboard.healthFair', '存在优化空间') : t('dev.dashboard.healthPoor', '需要重点关注')}
        </div>
        <div className="health-score-breakdown">
          <span>📁 {orphans.length} {t('dev.dashboard.orphanFiles', '孤立')}</span>
          <span>🔃 {duplicates.length} {t('dev.dashboard.dupGroups', '重复')}</span>
          <span>⚠️ {highRiskHotspots} {t('dev.dashboard.highRisk', '高风险')}</span>
        </div>
      </div>

      {/* Stats Overview */}
      <div className="stats-grid">
        <div className="stat-card">
          <div className="stat-value">{stats?.total_files ?? '-'}</div>
          <div className="stat-label">{t('dev.dashboard.totalFiles', '总文件数')}</div>
        </div>
        <div className="stat-card">
          <div className="stat-value">{stats?.script_count ?? '-'}</div>
          <div className="stat-label">{t('dev.dashboard.scripts', '脚本文件')}</div>
        </div>
        <div className="stat-card">
          <div className="stat-value">{stats?.class_count ?? '-'}</div>
          <div className="stat-label">{t('dev.dashboard.classes', '类/结构')}</div>
        </div>
        <div className="stat-card">
          <div className="stat-value">{stats?.method_count ?? '-'}</div>
          <div className="stat-label">{t('dev.dashboard.methods', '方法/函数')}</div>
        </div>
      </div>

      {/* Quick Insight Cards */}
      <div className="insight-grid">
        <div className="insight-card warning" onClick={() => navigate('/dev/redundancy')}>
          <h3>{t('dev.dashboard.orphanTitle', '孤立文件')}</h3>
          <div className="insight-value">{orphans.length}</div>
          <div className="insight-detail">
            {t('dev.dashboard.orphanSize', '可节省')} {formatSize(totalOrphanSize)}
          </div>
          <span className="insight-action">{t('common.viewDetails', '查看详情')} →</span>
        </div>

        <div className="insight-card danger" onClick={() => navigate('/dev/redundancy')}>
          <h3>{t('dev.dashboard.dupTitle', '重复文件')}</h3>
          <div className="insight-value">{duplicates.length} {t('dev.dashboard.groups', '组')}</div>
          <div className="insight-detail">
            {t('dev.dashboard.dupSize', '冗余占用')} {formatSize(totalDupSize)}
          </div>
          <span className="insight-action">{t('common.viewDetails', '查看详情')} →</span>
        </div>

        <div className="insight-card info" onClick={() => navigate('/dev/redundancy')}>
          <h3>{t('dev.dashboard.hotspotTitle', '热点依赖')}</h3>
          <div className="insight-value">{hotspots.length}</div>
          <div className="insight-detail">
            {hotspots.length > 0 ? `${t('dev.dashboard.maxDeps', '最高')} ${hotspots[0]?.in_degree} ${t('dev.dashboard.deps', '个依赖')}` : t('dev.dashboard.noHotspot', '无高风险依赖')}
          </div>
          <span className="insight-action">{t('common.viewDetails', '查看详情')} →</span>
        </div>

        <div className="insight-card accent" onClick={() => navigate('/dev/review')}>
          <h3>{t('dev.dashboard.reviewTitle', 'AI 代码审查')}</h3>
          <div className="insight-value">🤖</div>
          <div className="insight-detail">{t('dev.dashboard.reviewDesc', '逐行/架构/性能多维审查')}</div>
          <span className="insight-action">{t('dev.dashboard.startReview', '开始审查')} →</span>
        </div>
      </div>

      {/* Quick Links */}
      <div className="quick-links">
        <h2>{t('dev.dashboard.tools', '分析工具')}</h2>
        <div className="link-grid">
          <div className="link-card" onClick={() => navigate('/code')}>
            <span className="link-icon">🕸️</span>
            <span>{t('nav.code', '代码图谱')}</span>
          </div>
          <div className="link-card" onClick={() => navigate('/asset')}>
            <span className="link-icon">📦</span>
            <span>{t('nav.asset', '资源图谱')}</span>
          </div>
          <div className="link-card" onClick={() => navigate('/suspected')}>
            <span className="link-icon">🔗</span>
            <span>{t('nav.suspected', '疑似引用')}</span>
          </div>
          <div className="link-card" onClick={() => navigate('/hardcode')}>
            <span className="link-icon">📌</span>
            <span>{t('nav.hardcode', '硬编码')}</span>
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
