import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../store';

export default function SuspectedRefs() {
  const { t } = useTranslation();
  const { suspectedRefs, loadSuspectedRefs, promoteSuspectedRef, ignoreSuspectedRef } = useAppStore();
  const [filter, setFilter] = useState<string>('all');
  const [search, setSearch] = useState('');
  const [selected, setSelected] = useState<Set<string>>(new Set());

  useEffect(() => {
    loadSuspectedRefs();
  }, [loadSuspectedRefs]);

  const filtered = suspectedRefs.filter((ref) => {
    if (filter !== 'all' && ref.status.toLowerCase() !== filter) return false;
    if (search) {
      const q = search.toLowerCase();
      return ref.resource_path.toLowerCase().includes(q) ||
        ref.code_location.toLowerCase().includes(q);
    }
    return true;
  });

  const toggleSelect = (id: string) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const handleBatchConfirm = async () => {
    for (const id of selected) {
      await promoteSuspectedRef(id);
    }
    setSelected(new Set());
  };

  return (
    <div className="page-main">
      <div className="page-header">
        <div className="page-title">{t('suspected.title')}</div>
        <div className="page-actions">
          <button className="btn btn-primary" onClick={handleBatchConfirm} disabled={selected.size === 0}>
            ✓ {t('suspected.batchConfirm')} ({selected.size})
          </button>
          <button className="btn btn-ghost">{t('suspected.exportList')}</button>
        </div>
      </div>

      <div className="filter-bar">
        <input
          className="search-input"
          placeholder={t('suspected.searchPlaceholder')}
          value={search}
          onChange={(e) => setSearch(e.target.value)}
        />
        {['all', 'pending', 'confirmed', 'ignored'].map((f) => (
          <button
            key={f}
            className={`filter-btn${filter === f ? ' active' : ''}`}
            onClick={() => setFilter(f)}
          >
            {t(`suspected.filter${f.charAt(0).toUpperCase() + f.slice(1)}`)}
          </button>
        ))}
      </div>

      <div className="ref-grid">
        {filtered.map((ref) => (
          <div key={ref.id} className="ref-item">
            <input
              type="checkbox"
              className="ref-checkbox"
              checked={selected.has(ref.id)}
              onChange={() => toggleSelect(ref.id)}
            />
            <div className="ref-body">
              <div className="ref-resource">{ref.resource_path}</div>
              <div className="ref-location">
                {ref.code_location}:{ref.code_line}
              </div>
              <div className="ref-snippet" style={{ fontSize: 11, color: 'var(--text-dimmer)', fontFamily: 'monospace' }}>
                {ref.code_excerpt}
              </div>
              <div className="ref-method">{ref.load_method}</div>
            </div>
            <span className={`ref-confidence confidence-${getConfidenceLevel(ref.confidence)}`}>
              {Math.round(ref.confidence * 100)}%
            </span>
            <span className={`status-badge status-${getStatusClass(ref.status)}`}>
              {ref.status}
            </span>
            <div className="ref-actions">
              <button className="btn btn-primary" style={{ padding: '4px 12px', fontSize: 12 }}
                onClick={() => promoteSuspectedRef(ref.id)}>
                {t('suspected.confirm')}
              </button>
              <button className="btn btn-ghost" style={{ padding: '4px 12px', fontSize: 12 }}
                onClick={() => ignoreSuspectedRef(ref.id)}>
                {t('suspected.ignore')}
              </button>
            </div>
          </div>
        ))}
        {filtered.length === 0 && (
          <div style={{ textAlign: 'center', color: 'var(--text-dimmer)', padding: 40 }}>
            {suspectedRefs.length === 0
              ? t('overview.noProject')
              : `0 ${t('suspected.filterAll')}`}
          </div>
        )}
      </div>
    </div>
  );
}

function getConfidenceLevel(c: number): string {
  if (c >= 0.8) return 'high';
  if (c >= 0.5) return 'medium';
  return 'low';
}

function getStatusClass(status: string): string {
  switch (status.toLowerCase()) {
    case 'pending': return 'pending';
    case 'confirmed': return 'done';
    case 'ignored': return 'error';
    default: return 'pending';
  }
}
