import { useEffect, useState, useRef } from 'react';
import { useAppStore } from '../store';
import { useTranslation } from 'react-i18next';

type SortBy = 'size' | 'memory' | 'rating';
type Filter = 'all' | 'poor' | 'fair' | 'good';

const severityOrder: Record<string, number> = { Critical: 0, Warning: 1, Info: 2, Suggestion: 3 };

export default function ArtAssetAnalysis() {
  const { t } = useTranslation();
  const assetMetrics = useAppStore((s) => s.assetMetrics);
  const loadAssetMetrics = useAppStore((s) => s.loadAssetMetrics);
  const assetReviewResult = useAppStore((s) => s.assetReviewResult);
  const assetReviewLoading = useAppStore((s) => s.assetReviewLoading);
  const runAssetReview = useAppStore((s) => s.runAssetReview);
  const aiLiveLog = useAppStore((s) => s.aiLiveLog);
  const error = useAppStore((s) => s.error);
  const openFileLocation = useAppStore((s) => s.openFileLocation);
  const logEndRef = useRef<HTMLDivElement>(null);

  const [sortBy, setSortBy] = useState<SortBy>('size');
  const [filter, setFilter] = useState<Filter>('all');

  useEffect(() => {
    loadAssetMetrics();
  }, []);

  useEffect(() => {
    logEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [aiLiveLog.length]);

  const filtered = assetMetrics
    .filter((m) => filter === 'all' || m.performance_rating === filter)
    .sort((a, b) => {
      if (sortBy === 'size') return b.file_size_bytes - a.file_size_bytes;
      if (sortBy === 'memory') return (b.estimated_memory_bytes ?? 0) - (a.estimated_memory_bytes ?? 0);
      const order: Record<string, number> = { poor: 0, fair: 1, good: 2 };
      return (order[a.performance_rating ?? 'good'] ?? 3) - (order[b.performance_rating ?? 'good'] ?? 3);
    });

  // Asset review findings
  const findings = assetReviewResult?.findings ?? [];
  const sortedFindings = [...findings].sort(
    (a, b) => (severityOrder[a.severity] ?? 9) - (severityOrder[b.severity] ?? 9)
  );
  const hasStructured = sortedFindings.length > 0 && sortedFindings[0].category !== 'AI原始回复';

  return (
    <div className="asset-analysis-page">
      <h1 className="page-title">{t('art.analysis.title', '资源指标分析')}</h1>

      {/* Controls */}
      <div className="analysis-toolbar">
        <div className="filter-group">
          <button className={`tab ${filter === 'all' ? 'active' : ''}`} onClick={() => setFilter('all')}>
            {t('art.analysis.all', '全部')} ({assetMetrics.length})
          </button>
          <button className={`tab ${filter === 'poor' ? 'active' : ''}`} onClick={() => setFilter('poor')}>
            ⚠️ {t('art.analysis.poor', '差')} ({assetMetrics.filter((m) => m.performance_rating === 'poor').length})
          </button>
          <button className={`tab ${filter === 'fair' ? 'active' : ''}`} onClick={() => setFilter('fair')}>
            ⚡ {t('art.analysis.fair', '一般')} ({assetMetrics.filter((m) => m.performance_rating === 'fair').length})
          </button>
          <button className={`tab ${filter === 'good' ? 'active' : ''}`} onClick={() => setFilter('good')}>
            ✅ {t('art.analysis.good', '良好')} ({assetMetrics.filter((m) => m.performance_rating === 'good').length})
          </button>
        </div>
        <div className="filter-group" style={{ marginLeft: 'auto' }}>
          <label style={{ fontSize: 12, color: 'var(--text-dimmer)', marginRight: 6 }}>{t('art.analysis.sortBy', '排序')}</label>
          <select value={sortBy} onChange={(e) => setSortBy(e.target.value as SortBy)} className="file-select" style={{ width: 'auto', minWidth: 100 }}>
            <option value="size">{t('art.analysis.bySize', '文件大小')}</option>
            <option value="memory">{t('art.analysis.byMemory', '内存占用')}</option>
            <option value="rating">{t('art.analysis.byRating', '性能评级')}</option>
          </select>
        </div>
        <button
          className="btn primary"
          onClick={runAssetReview}
          disabled={assetReviewLoading}
        >
          {assetReviewLoading
            ? t('art.analysis.aiRunning', 'AI分析中...')
            : t('art.analysis.aiOptimize', '🤖 AI 优化建议')}
        </button>
      </div>

      {/* Live AI Log */}
      {assetReviewLoading && (
        <div className="review-ai-log" style={{ marginBottom: 20 }}>
          <div className="ai-log-header">{t('art.analysis.aiLog', 'AI 实时日志')}</div>
          <div className="ai-log-content">
            {aiLiveLog.length === 0
              ? <span style={{ color: 'var(--text-dimmer)' }}>{t('art.analysis.waitingAi', '等待 AI 响应...')}</span>
              : aiLiveLog.map((line, i) => <div key={i}>{line}</div>)
            }
            <div ref={logEndRef} />
          </div>
        </div>
      )}

      {error && !assetReviewLoading && (
        <div className="ai-error" style={{ marginBottom: 20 }}>
          {error}
        </div>
      )}

      {/* AI Review Results */}
      {!assetReviewLoading && assetReviewResult && (
        <div style={{ marginBottom: 24 }}>
          {/* Summary */}
          <div className="findings-summary">
            <span className="summary-label">{assetReviewResult.summary}</span>
            <span style={{ fontSize: 11, color: 'var(--text-dimmer)' }}>{assetReviewResult.timestamp}</span>
          </div>

          {hasStructured ? (
            sortedFindings.map((f) => (
              <div
                key={f.id}
                className={`finding-card severity-${f.severity.toLowerCase()}`}
                onClick={() => f.file_path && openFileLocation(f.file_path)}
                style={{ cursor: f.file_path ? 'pointer' : 'default' }}
              >
                <div className="finding-card-header">
                  <span className={`severity-badge ${f.severity === 'Critical' ? 'error' : f.severity === 'Warning' ? 'warning' : f.severity === 'Suggestion' ? 'hint' : 'info'}`}>
                    {f.severity}
                  </span>
                  <span className="finding-category">{f.category}</span>
                  {f.file_path && <span className="finding-location">{f.file_path}</span>}
                </div>
                <div className="finding-card-body">{f.message}</div>
                {f.suggestion && (
                  <div className="finding-card-suggestion">{f.suggestion}</div>
                )}
              </div>
            ))
          ) : sortedFindings.length > 0 && sortedFindings[0].category === 'AI原始回复' ? (
            <div className="raw-response-panel">
              <div className="raw-header">{t('art.analysis.rawResponse', 'AI 原始回复（未能解析为结构化结果）')}</div>
              <div className="raw-body">{sortedFindings[0].message}</div>
            </div>
          ) : assetReviewResult.raw_response ? (
            <div className="raw-response-panel">
              <div className="raw-header">{t('art.analysis.rawResponse', 'AI 原始回复')}</div>
              <div className="raw-body">{assetReviewResult.raw_response}</div>
            </div>
          ) : (
            <div className="empty-state" style={{ marginTop: 8 }}>✅ {t('art.analysis.allGood', '所有资源表现良好')}</div>
          )}
        </div>
      )}

      {/* Metrics Table */}
      <div className="result-list">
        <div className="list-header">
          <span className="col-path">{t('common.filePath', '文件路径')}</span>
          <span className="col-type">{t('common.type', '类型')}</span>
          <span className="col-size">{t('common.size', '大小')}</span>
          <span className="col-dims">{t('art.analysis.dimensions', '尺寸')}</span>
          <span className="col-memory">{t('art.analysis.memory', '显存')}</span>
          <span className="col-rating">{t('art.analysis.rating', '评级')}</span>
        </div>
        {filtered.map((m) => {
          const ext = m.file_path.split('.').pop()?.toLowerCase() ?? '';
          const kind = ['png', 'jpg', 'jpeg', 'tga', 'bmp', 'psd', 'tif', 'tiff', 'exr', 'hdr'].includes(ext) ? '纹理'
            : ['wav', 'mp3', 'ogg', 'aif', 'aiff', 'flac'].includes(ext) ? '音频'
            : ['fbx', 'obj', 'blend', 'dae', 'gltf', 'glb'].includes(ext) ? '模型'
            : ext;
          return (
            <div
              key={m.node_id}
              className="list-row"
              onClick={() => openFileLocation(m.file_path)}
            >
              <span className="col-path" title={m.file_path}>{m.file_path}</span>
              <span className="col-type"><span className="badge asset-kind">{kind}</span></span>
              <span className="col-size">{formatSize(m.file_size_bytes)}</span>
              <span className="col-dims">
                {m.texture_width && m.texture_height
                  ? `${m.texture_width}×${m.texture_height}`
                  : m.duration_seconds
                    ? `${m.duration_seconds.toFixed(1)}s`
                    : '-'}
              </span>
              <span className="col-memory">
                {m.estimated_memory_bytes ? formatSize(m.estimated_memory_bytes) : '-'}
              </span>
              <span className="col-rating">
                <span className={`rating-badge ${m.performance_rating ?? 'unknown'}`}>
                  {m.performance_rating === 'poor' ? '⚠️ 差'
                    : m.performance_rating === 'fair' ? '⚡ 一般'
                    : m.performance_rating === 'good' ? '✅ 良好'
                    : '-'}
                </span>
              </span>
            </div>
          );
        })}
        {filtered.length === 0 && (
          <div className="empty-state">{t('art.analysis.noResults', '没有符合条件的资源')}</div>
        )}
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
