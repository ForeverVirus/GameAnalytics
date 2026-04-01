import { useEffect, useState } from 'react';
import { useAppStore } from '../store';
import { useTranslation } from 'react-i18next';

type Tab = 'orphans' | 'duplicates' | 'hotspots';

export default function DevRedundancy() {
  const { t } = useTranslation();
  const [tab, setTab] = useState<Tab>('orphans');
  const orphans = useAppStore((s) => s.orphans);
  const duplicates = useAppStore((s) => s.duplicates);
  const hotspots = useAppStore((s) => s.hotspots);
  const loadOrphans = useAppStore((s) => s.loadOrphans);
  const loadDuplicates = useAppStore((s) => s.loadDuplicates);
  const loadHotspots = useAppStore((s) => s.loadHotspots);
  const openFileLocation = useAppStore((s) => s.openFileLocation);

  useEffect(() => {
    loadOrphans();
    loadDuplicates();
    loadHotspots();
  }, []);

  return (
    <div className="redundancy-page">
      <h1 className="page-title">{t('dev.redundancy.title', '冗余检测')}</h1>

      <div className="tab-bar">
        <button className={`tab ${tab === 'orphans' ? 'active' : ''}`} onClick={() => setTab('orphans')}>
          {t('dev.redundancy.orphans', '孤立文件')} ({orphans.length})
        </button>
        <button className={`tab ${tab === 'duplicates' ? 'active' : ''}`} onClick={() => setTab('duplicates')}>
          {t('dev.redundancy.duplicates', '重复文件')} ({duplicates.length})
        </button>
        <button className={`tab ${tab === 'hotspots' ? 'active' : ''}`} onClick={() => setTab('hotspots')}>
          {t('dev.redundancy.hotspots', '热点依赖')} ({hotspots.length})
        </button>
      </div>

      <div className="tab-content">
        {tab === 'orphans' && <OrphanList items={orphans} onOpen={openFileLocation} t={t} />}
        {tab === 'duplicates' && <DuplicateList groups={duplicates} onOpen={openFileLocation} t={t} />}
        {tab === 'hotspots' && <HotspotList items={hotspots} onOpen={openFileLocation} t={t} />}
      </div>
    </div>
  );
}

function OrphanList({ items, onOpen, t }: { items: any[]; onOpen: (fp: string) => void; t: any }) {
  if (items.length === 0) {
    return <div className="empty-state">{t('dev.redundancy.noOrphans', '🎉 没有发现孤立文件')}</div>;
  }
  return (
    <div className="result-list">
      <div className="list-header">
        <span className="col-name">{t('common.fileName', '文件名')}</span>
        <span className="col-type">{t('common.type', '类型')}</span>
        <span className="col-size">{t('common.size', '大小')}</span>
        <span className="col-action">{t('common.suggestion', '建议')}</span>
      </div>
      {items.map((item) => (
        <div key={item.node_id} className="list-row" onClick={() => item.file_path && onOpen(item.file_path)}>
          <span className="col-name" title={item.file_path || ''}>{item.node_name}</span>
          <span className="col-type">
            <span className={`badge ${item.node_type.toLowerCase()}`}>{item.node_type}</span>
            {item.asset_kind && <span className="badge asset-kind">{item.asset_kind}</span>}
          </span>
          <span className="col-size">{formatSize(item.file_size_bytes)}</span>
          <span className="col-action suggestion-text">{item.suggestion}</span>
        </div>
      ))}
    </div>
  );
}

function DuplicateList({ groups, onOpen, t }: { groups: any[]; onOpen: (fp: string) => void; t: any }) {
  if (groups.length === 0) {
    return <div className="empty-state">{t('dev.redundancy.noDuplicates', '🎉 没有发现重复文件')}</div>;
  }
  return (
    <div className="result-list">
      {groups.map((group) => (
        <div key={group.group_id} className="dup-group">
          <div className="dup-header">
            <span className="dup-id">{group.group_id}</span>
            {group.asset_kind && <span className="badge asset-kind">{group.asset_kind}</span>}
            <span className="dup-count">{group.files.length} {t('dev.redundancy.files', '个文件')}</span>
            <span className="dup-size">{t('dev.redundancy.totalWaste', '冗余占用')} {formatSize(group.total_size)}</span>
          </div>
          <div className="dup-files">
            {group.files.map((f: any) => (
              <div key={f.node_id} className="dup-file" onClick={() => onOpen(f.file_path)}>
                <span className="file-path">{f.file_path}</span>
                <span className="file-size">{formatSize(f.file_size)}</span>
              </div>
            ))}
          </div>
        </div>
      ))}
    </div>
  );
}

function HotspotList({ items, onOpen, t }: { items: any[]; onOpen: (fp: string) => void; t: any }) {
  if (items.length === 0) {
    return <div className="empty-state">{t('dev.redundancy.noHotspots', '🎉 没有发现热点依赖')}</div>;
  }
  return (
    <div className="result-list">
      <div className="list-header">
        <span className="col-name">{t('common.fileName', '文件名')}</span>
        <span className="col-type">{t('common.type', '类型')}</span>
        <span className="col-deps">{t('dev.redundancy.dependents', '被依赖数')}</span>
        <span className="col-risk">{t('dev.redundancy.risk', '风险等级')}</span>
      </div>
      {items.map((item) => (
        <div key={item.node_id} className="list-row" onClick={() => item.file_path && onOpen(item.file_path)}>
          <span className="col-name" title={item.file_path || ''}>{item.node_name}</span>
          <span className="col-type">
            <span className={`badge ${item.node_type.toLowerCase()}`}>{item.node_type}</span>
          </span>
          <span className="col-deps">{item.in_degree}</span>
          <span className="col-risk">
            <span className={`risk-badge ${item.risk_level.toLowerCase()}`}>{item.risk_level}</span>
          </span>
        </div>
      ))}
    </div>
  );
}

function formatSize(bytes: number): string {
  if (bytes === 0) return '0 B';
  const units = ['B', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(1024));
  return (bytes / Math.pow(1024, i)).toFixed(1) + ' ' + units[i];
}
