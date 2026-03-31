import { useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../store';

const phases = ['scan'] as const;

export default function ProgressModal() {
  const { t } = useTranslation();
  const {
    showProgress,
    progressPhase,
    progressCurrent,
    progressTotal,
    progressMessage,
    progressDone,
    dismissProgress,
    aiLogs,
  } = useAppStore();

  const logEndRef = useRef<HTMLDivElement>(null);

  // Auto-scroll log to bottom
  useEffect(() => {
    logEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [aiLogs]);

  if (!showProgress) return null;

  const getPhaseLabel = (phase: string) => {
    switch (phase) {
      case 'scan': return t('progress.scan');
      default: return phase;
    }
  };

  const getPhaseStatus = (phase: string) => {
    const currentIdx = phases.indexOf(progressPhase as typeof phases[number]);
    const phaseIdx = phases.indexOf(phase as typeof phases[number]);
    if (phaseIdx < currentIdx) return 'done';
    if (phaseIdx === currentIdx) return progressDone ? 'done' : 'active';
    return 'pending';
  };

  const percent = progressTotal > 0 ? Math.round((progressCurrent / progressTotal) * 100) : 0;
  const showLogs = progressDone && aiLogs.length > 0;

  return (
    <div className="progress-overlay">
      <div className={`progress-card ${showLogs ? 'progress-card-wide' : ''}`}>
        <div className="progress-title">
          {progressDone ? t('progress.complete') : t('progress.analyzing')}
        </div>

        {/* Phase indicators */}
        <div className="progress-phases">
          {phases.map((phase) => {
            const status = getPhaseStatus(phase);
            return (
              <div key={phase} className={`progress-phase ${status}`}>
                <div className="progress-phase-dot">
                  {status === 'done' ? '✓' : status === 'active' ? '●' : '○'}
                </div>
                <div className="progress-phase-label">{getPhaseLabel(phase)}</div>
              </div>
            );
          })}
        </div>

        {/* Progress bar */}
        {!progressDone && (
          <div className="progress-bar-container">
            <div className="progress-bar-track">
              <div className="progress-bar-fill" style={{ width: `${percent}%` }} />
            </div>
            <div className="progress-bar-text">{percent}%</div>
          </div>
        )}

        {/* Current step message */}
        <div className="progress-message">{progressMessage}</div>

        {/* AI Log panel */}
        {showLogs && (
          <div className="ai-log-panel">
            <div className="ai-log-header">{t('progress.aiLog')}</div>
            <div className="ai-log-content">
              {aiLogs.map((line, i) => (
                <div key={i} className={`ai-log-line${line.includes('[stderr]') ? ' ai-log-stderr' : ''}${line.startsWith('━') ? ' ai-log-separator' : ''}`}>
                  {line}
                </div>
              ))}
              <div ref={logEndRef} />
            </div>
          </div>
        )}

        {/* Done button */}
        {progressDone && (
          <button className="btn btn-primary progress-done-btn" onClick={dismissProgress}>
            {t('common.confirm')}
          </button>
        )}
      </div>
    </div>
  );
}
