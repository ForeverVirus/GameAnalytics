import { useState, useRef, useEffect } from 'react';
import { useAppStore } from '../store';

export default function AiStatusBar() {
  const { aiLoading, aiNodeId, aiLiveLog } = useAppStore();
  const [expanded, setExpanded] = useState(false);
  const logContainerRef = useRef<HTMLDivElement>(null);

  // Auto-scroll log to bottom
  useEffect(() => {
    if (expanded && logContainerRef.current) {
      logContainerRef.current.scrollTop = logContainerRef.current.scrollHeight;
    }
  }, [aiLiveLog, expanded]);

  // Auto-collapse on start
  useEffect(() => {
    if (aiLoading) setExpanded(false);
  }, [aiLoading]);

  if (!aiLoading && aiLiveLog.length === 0) return null;

  const lastLine = aiLiveLog.length > 0 ? aiLiveLog[aiLiveLog.length - 1] : '准备中...';
  const shortName = aiNodeId?.split(/[\\/]/).pop()?.split('::').pop() ?? aiNodeId ?? '';

  return (
    <div className="ai-status-bar">
      <div className="ai-status-header" onClick={(e) => { e.stopPropagation(); setExpanded(prev => !prev); }}>
        <div className="ai-status-left">
          {aiLoading && <span className="ai-status-spinner" />}
          <span className="ai-status-label">
            {aiLoading ? '🤖 AI 分析中' : '✓ 分析完成'}
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
