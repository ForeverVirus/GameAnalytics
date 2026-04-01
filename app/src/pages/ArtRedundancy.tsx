import { useEffect, useState } from 'react';
import { useAppStore } from '../store';
import { useTranslation } from 'react-i18next';

type Tab = 'orphans' | 'duplicates';

export default function ArtRedundancy() {
  const { t } = useTranslation();
  const [tab, setTab] = useState<Tab>('orphans');
  const orphans = useAppStore((s) => s.orphans);
  const duplicates = useAppStore((s) => s.duplicates);
  const loadOrphans = useAppStore((s) => s.loadOrphans);
  const loadDuplicates = useAppStore((s) => s.loadDuplicates);
  const openFileLocation = useAppStore((s) => s.openFileLocation);

  useEffect(() => {
    loadOrphans();
    loadDuplicates();
  }, []);

  // Filter for asset-only items
  const assetOrphans = orphans.filter((o) => o.node_type === 'Asset');
  const assetDuplicates = duplicates.filter((g) => g.asset_kind !== null);

  return (
    <div className="redundancy-page">
      <h1 className="page-title">{t('art.redundancy.title', '冗余资源')}</h1>

      <div className="tab-bar">
        <button className={`tab ${tab === 'orphans' ? 'active' : ''}`} onClick={() => setTab('orphans')}>
          {t('art.redundancy.orphans', '孤立资源')} ({assetOrphans.length})
        </button>
        <button className={`tab ${tab === 'duplicates' ? 'active' : ''}`} onClick={() => setTab('duplicates')}>
          {t('art.redundancy.duplicates', '重复资源')} ({assetDuplicates.length})
        </button>
      </div>

      <div className="tab-content">
        {tab === 'orphans' && (
          assetOrphans.length === 0 ? (
            <div className="empty-state">{t('art.redundancy.noOrphans', '🎉 没有发现孤立资源')}</div>
          ) : (
            <div className="result-list">
              <div className="list-header">
                <span className="col-name">{t('common.fileName', '文件名')}</span>
                <span className="col-type">{t('common.type', '类型')}</span>
                <span className="col-size">{t('common.size', '大小')}</span>
                <span className="col-action">{t('common.suggestion', '建议')}</span>
              </div>
              {assetOrphans.map((item) => (
                <div key={item.node_id} className="list-row" onClick={() => item.file_path && openFileLocation(item.file_path)}>
                  <span className="col-name" title={item.file_path || ''}>{item.node_name}</span>
                  <span className="col-type">
                    {item.asset_kind && <span className="badge asset-kind">{item.asset_kind}</span>}
                  </span>
                  <span className="col-size">{formatSize(item.file_size_bytes)}</span>
                  <span className="col-action suggestion-text">{item.suggestion}</span>
                </div>
              ))}
            </div>
          )
        )}

        {tab === 'duplicates' && (
          assetDuplicates.length === 0 ? (
            <div className="empty-state">{t('art.redundancy.noDuplicates', '🎉 没有发现重复资源')}</div>
          ) : (
            <div className="result-list">
              {assetDuplicates.map((group) => (
                <div key={group.group_id} className="dup-group">
                  <div className="dup-header">
                    <span className="dup-id">{group.group_id}</span>
                    {group.asset_kind && <span className="badge asset-kind">{group.asset_kind}</span>}
                    <span className="dup-count">{group.files.length} {t('art.redundancy.files', '个文件')}</span>
                    <span className="dup-size">{formatSize(group.total_size)}</span>
                  </div>
                  <div className="dup-files">
                    {group.files.map((f: any) => (
                      <div key={f.node_id} className="dup-file" onClick={() => openFileLocation(f.file_path)}>
                        <span className="file-path">{f.file_path}</span>
                        <span className="file-size">{formatSize(f.file_size)}</span>
                      </div>
                    ))}
                  </div>
                </div>
              ))}
            </div>
          )
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
