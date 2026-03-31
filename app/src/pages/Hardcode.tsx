import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../store';

export default function Hardcode() {
  const { t } = useTranslation();
  const { hardcodeFindings, loadHardcodeFindings } = useAppStore();
  const [filter, setFilter] = useState<string>('all');
  const [search, setSearch] = useState('');

  useEffect(() => {
    loadHardcodeFindings();
  }, [loadHardcodeFindings]);

  const categories = ['all', 'Path', 'Url', 'MagicNumber', 'Color', 'StringLiteral'];
  const categoryLabels: Record<string, string> = {
    all: t('hardcode.catAll'),
    Path: t('hardcode.catPath'),
    Url: t('hardcode.catUrl'),
    MagicNumber: t('hardcode.catMagic'),
    Color: t('hardcode.catColor'),
    StringLiteral: t('hardcode.catString'),
  };

  const filtered = hardcodeFindings.filter((f) => {
    if (filter !== 'all' && f.category !== filter) return false;
    if (search) {
      const q = search.toLowerCase();
      return f.value.toLowerCase().includes(q) || f.file_path.toLowerCase().includes(q);
    }
    return true;
  });

  // Group by file
  const groups = new Map<string, typeof filtered>();
  for (const item of filtered) {
    const list = groups.get(item.file_path) ?? [];
    list.push(item);
    groups.set(item.file_path, list);
  }

  return (
    <div className="page-main">
      <div className="page-header">
        <div className="page-title">{t('hardcode.title')}</div>
        <div className="page-actions">
          <button className="btn btn-ghost">{t('hardcode.exportList')}</button>
        </div>
      </div>

      <div className="filter-bar">
        <input
          className="search-input"
          placeholder={t('hardcode.searchPlaceholder')}
          value={search}
          onChange={(e) => setSearch(e.target.value)}
        />
        {categories.map((cat) => (
          <button
            key={cat}
            className={`filter-btn${filter === cat ? ' active' : ''}`}
            onClick={() => setFilter(cat)}
          >
            {categoryLabels[cat] ?? cat}
          </button>
        ))}
      </div>

      {Array.from(groups.entries()).map(([file, items]) => (
        <div key={file} className="hardcode-group">
          <div className="hardcode-group-header">
            <span>📄 {file.split(/[\\/]/).pop()}</span>
            <span className="count">{items.length}</span>
            <span style={{ flex: 1 }} />
            <span style={{ fontSize: 11, color: 'var(--text-dimmer)' }}>{file}</span>
          </div>
          {items.map((item) => (
            <div key={item.id} className="hardcode-item">
              <span className="line-num">L{item.line_number}</span>
              <div className="code-snippet">
                {item.code_excerpt.split(item.value).map((part, i, arr) => (
                  <span key={i}>
                    {part}
                    {i < arr.length - 1 && <span className="highlight">{item.value}</span>}
                  </span>
                ))}
              </div>
              <span className={`severity severity-${item.severity.toLowerCase()}`}>
                {item.severity}
              </span>
            </div>
          ))}
        </div>
      ))}

      {groups.size === 0 && (
        <div style={{ textAlign: 'center', color: 'var(--text-dimmer)', padding: 40 }}>
          {hardcodeFindings.length === 0
            ? t('overview.noProject')
            : `0 ${t('hardcode.catAll')}`}
        </div>
      )}
    </div>
  );
}
