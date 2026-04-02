import { useState, useRef, useEffect } from 'react';
import { useAppStore } from '../store';
import { useTranslation } from 'react-i18next';

export default function AiStatusBar() {
  const { t } = useTranslation();
  const { aiLoading, aiNodeId, aiLiveLog, reviewLoading, assetReviewLoading, profilerLoading, error } = useAppStore();
  const [expanded, setExpanded] = useState(false);
  const logContainerRef = useRef<HTMLDivElement>(null);
  const isRunning = aiLoading || reviewLoading || assetReviewLoading || profilerLoading;
  const hasError = !isRunning && !!error && aiLiveLog.length > 0;

  // Auto-scroll log to bottom
  useEffect(() => {
    if (expanded && logContainerRef.current) {
      logContainerRef.current.scrollTop = logContainerRef.current.scrollHeight;
    }
  }, [aiLiveLog, expanded]);

  // Auto-expand on start so CLI logs are immediately visible.
  useEffect(() => {
    if (isRunning) setExpanded(true);
  }, [isRunning]);

  if (!isRunning && aiLiveLog.length === 0) return null;

  const lastLine = aiLiveLog.length > 0 ? aiLiveLog[aiLiveLog.length - 1] : t('common.loading', '加载中...');
  const shortName = aiLoading
    ? (aiNodeId?.split(/[\\/]/).pop()?.split('::').pop() ?? aiNodeId ?? '')
    : reviewLoading
      ? t('nav.codeReview', '代码审查')
      : assetReviewLoading
        ? t('nav.assetAnalysis', '资源分析')
        : profilerLoading
          ? t('nav.performance', '性能分析')
        : '';

  return (
    <div className="ai-status-bar">
      <div className="ai-status-header" onClick={(e) => { e.stopPropagation(); setExpanded(prev => !prev); }}>
        <div className="ai-status-left">
          {isRunning && <span className="ai-status-spinner" />}
          <span className="ai-status-label">
            {isRunning
              ? `🤖 ${t('graph.aiAnalyzing', '分析中...')}`
              : hasError
                ? `! ${t('progress.failed', '分析失败')}`
                : `✓ ${t('progress.complete', '分析完成')}`}
          </span>
          <span className="ai-status-node">{shortName}</span>
        </div>
        <div className="ai-status-right">
          <span className="ai-status-msg">{lastLine.length > 60 ? lastLine.slice(0, 58) + '…' : lastLine}</span>
          <span className="ai-status-toggle">{expanded ? '▼' : '▲'}</span>
        </div>
      </div>
      {expanded && (
        <div className="ai-status-log" ref={logContainerRef}>
          {aiLiveLog.map((line, i) => (
            <div key={i} className="ai-status-log-line">{line}</div>
          ))}
        </div>
      )}
    </div>
  );
}
