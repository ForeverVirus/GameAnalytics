import { useState, useRef, useEffect } from 'react';
import { useAppStore } from '../store';
import { useTranslation } from 'react-i18next';

type ReviewTab = 'line' | 'architecture' | 'performance';

const severityOrder: Record<string, number> = { Critical: 0, Warning: 1, Info: 2, Suggestion: 3 };

export default function CodeReview() {
  const { t } = useTranslation();
  const [tab, setTab] = useState<ReviewTab>('line');
  const [selectedNodeId, setSelectedNodeId] = useState('');
  const codeGraph = useAppStore((s) => s.codeGraph);
  const loadCodeGraph = useAppStore((s) => s.loadCodeGraph);
  const reviewResults = useAppStore((s) => s.reviewResults);
  const reviewLoading = useAppStore((s) => s.reviewLoading);
  const runCodeReview = useAppStore((s) => s.runCodeReview);
  const runProjectCodeReview = useAppStore((s) => s.runProjectCodeReview);
  const clearReviewResults = useAppStore((s) => s.clearReviewResults);
  const aiLiveLog = useAppStore((s) => s.aiLiveLog);
  const error = useAppStore((s) => s.error);
  const openFileLocation = useAppStore((s) => s.openFileLocation);
  const logEndRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    loadCodeGraph();
  }, [loadCodeGraph]);

  useEffect(() => {
    logEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [aiLiveLog.length]);

  const codeFiles = (codeGraph?.nodes ?? [])
    .filter((n) => n.node_type === 'CodeFile')
    .sort((a, b) => a.name.localeCompare(b.name));

  const tabType = tab === 'line' ? 'Line' : tab === 'architecture' ? 'Architecture' : 'Performance';
  const currentResults = reviewResults.filter((r) => r.review_type === tabType);

  const handleRunReview = () => {
    if (!selectedNodeId) return;
    runCodeReview(selectedNodeId, tab);
  };

  const handleRunProjectReview = () => {
    runProjectCodeReview(tab);
  };

  return (
    <div className="review-page">
      <h1 className="page-title">{t('dev.review.title', 'AI 代码审查')}</h1>

      {/* File Selector */}
      <div className="review-controls">
        <select
          className="file-select"
          value={selectedNodeId}
          onChange={(e) => setSelectedNodeId(e.target.value)}
        >
          <option value="">{t('dev.review.selectFile', '-- 选择文件 --')}</option>
          {codeFiles.map((f) => (
            <option key={f.id} value={f.id}>{f.file_path || f.name}</option>
          ))}
        </select>
        <button
          className="btn primary"
          onClick={handleRunReview}
          disabled={!selectedNodeId || reviewLoading}
        >
          {reviewLoading ? t('dev.review.running', '审查中...') : t('dev.review.run', '开始审查')}
        </button>
        <button
          className="btn secondary"
          onClick={handleRunProjectReview}
          disabled={codeFiles.length === 0 || reviewLoading}
        >
          {reviewLoading ? t('dev.review.runningAll', '全量分析中...') : t('dev.review.runAll', '全量代码分析')}
        </button>
        <button className="btn secondary" onClick={clearReviewResults}>
          {t('dev.review.clear', '清空结果')}
        </button>
      </div>

      {/* Review Type Tabs */}
      <div className="tab-bar">
        <button className={`tab ${tab === 'line' ? 'active' : ''}`} onClick={() => setTab('line')}>
          {t('dev.review.lineTab', '📝 逐行审查')}
        </button>
        <button className={`tab ${tab === 'architecture' ? 'active' : ''}`} onClick={() => setTab('architecture')}>
          {t('dev.review.archTab', '🏗️ 架构审查')}
        </button>
        <button className={`tab ${tab === 'performance' ? 'active' : ''}`} onClick={() => setTab('performance')}>
          {t('dev.review.perfTab', '⚡ 性能审查')}
        </button>
      </div>

      {/* Live AI Log — always visible during loading */}
      {reviewLoading && (
        <div className="review-ai-log">
          <div className="ai-log-header">{t('dev.review.aiLog', 'AI 实时日志')}</div>
          <div className="ai-log-content">
            {aiLiveLog.length === 0
              ? <span style={{ color: 'var(--text-dimmer)' }}>{t('dev.review.waitingAi', '等待 AI 响应...')}</span>
              : aiLiveLog.map((line, i) => <div key={i}>{line}</div>)
            }
            <div ref={logEndRef} />
          </div>
        </div>
      )}

      {error && !reviewLoading && (
        <div className="ai-error" style={{ marginTop: 12 }}>
          {error}
        </div>
      )}

      {/* Results */}
      <div className="review-results" style={{ marginTop: 20 }}>
        {currentResults.length === 0 && !reviewLoading && (
          <div className="empty-state">
            {t('dev.review.noResults', '选择文件并点击"开始审查"以获取AI代码审查结果')}
          </div>
        )}

        {currentResults.map((result) => {
          const sorted = [...result.findings].sort(
            (a, b) => (severityOrder[a.severity] ?? 9) - (severityOrder[b.severity] ?? 9)
          );
          const resultFile = sorted[0]?.file_path || result.node_id;
          const counts = {
            critical: sorted.filter((f) => f.severity === 'Critical').length,
            warning: sorted.filter((f) => f.severity === 'Warning').length,
            info: sorted.filter((f) => f.severity === 'Info').length,
            suggestion: sorted.filter((f) => f.severity === 'Suggestion').length,
          };
          const hasStructuredFindings = sorted.length > 0 && sorted[0].category !== 'AI原始回复';

          return (
            <div key={`${result.node_id}-${result.review_type}-${result.timestamp}`} style={{ marginBottom: 24 }}>
              {/* Summary Bar */}
              <div className="findings-summary">
                <span className="finding-category" title={resultFile}>{resultFile}</span>
                <span className="summary-label">{result.summary}</span>
                {counts.critical > 0 && <span className="summary-count count-critical">{counts.critical} Critical</span>}
                {counts.warning > 0 && <span className="summary-count count-warning">{counts.warning} Warning</span>}
                {counts.info > 0 && <span className="summary-count count-info">{counts.info} Info</span>}
                {counts.suggestion > 0 && <span className="summary-count count-suggestion">{counts.suggestion} Suggestion</span>}
                <span style={{ fontSize: 11, color: 'var(--text-dimmer)' }}>{result.timestamp}</span>
              </div>

              {/* Finding Cards */}
              {hasStructuredFindings ? (
                sorted.map((f) => (
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
                      {f.line_number && (
                        <span className="finding-location">
                          L{f.line_number}{f.line_end ? `-${f.line_end}` : ''}
                        </span>
                      )}
                    </div>
                    <div className="finding-card-body">{f.message}</div>
                    {f.suggestion && (
                      <div className="finding-card-suggestion">{f.suggestion}</div>
                    )}
                  </div>
                ))
              ) : sorted.length > 0 && sorted[0].category === 'AI原始回复' ? (
                /* Raw response fallback — AI returned non-JSON */
                <div className="raw-response-panel">
                  <div className="raw-header">{t('dev.review.rawResponse', 'AI 原始回复（未能解析为结构化结果）')}</div>
                  <div className="raw-body">{sorted[0].message}</div>
                </div>
              ) : result.raw_response ? (
                /* Has raw_response but no findings */
                <div className="raw-response-panel">
                  <div className="raw-header">{t('dev.review.rawResponse', 'AI 原始回复')}</div>
                  <div className="raw-body">{result.raw_response}</div>
                </div>
              ) : (
                <div className="empty-state" style={{ marginTop: 8 }}>✅ {t('dev.review.noIssues', '未发现问题')}</div>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}
