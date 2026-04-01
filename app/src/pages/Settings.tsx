import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../store';
import { api, type CliStatus } from '../api/tauri';

const CLI_OPTIONS = [
  { id: 'claude', name: 'Claude CLI', icon: '🟣' },
  { id: 'gemini', name: 'Gemini CLI', icon: '🔵' },
  { id: 'codex', name: 'Codex CLI', icon: '🟢' },
  { id: 'copilot', name: 'Copilot CLI', icon: '⚪' },
];

const EMPTY_CLI_STATUSES: Record<string, CliStatus> = Object.fromEntries(
  CLI_OPTIONS.map((cli) => [cli.id, { available: false, resolved_path: null }]),
);

// Per-CLI model options
const CLI_MODELS: Record<string, { id: string; name: string }[]> = {
  claude: [
    { id: '', name: 'Default' },
    { id: 'claude-sonnet-4-6', name: 'Claude Sonnet 4.6' },
    { id: 'claude-opus-4-6', name: 'Claude Opus 4.6' },
    { id: 'claude-haiku-4-5', name: 'Claude Haiku 4.5' },
  ],
  codex: [
    { id: '', name: 'Default' },
    { id: 'gpt-5.4', name: 'GPT-5.4' },
    { id: 'gpt-5.4-mini', name: 'GPT-5.4 Mini' },
    { id: 'gpt-5.4-nano', name: 'GPT-5.4 Nano' },
  ],
  gemini: [
    { id: '', name: 'Default' },
    { id: 'gemini-2.5-pro', name: 'Gemini 2.5 Pro' },
    { id: 'gemini-2.5-flash', name: 'Gemini 2.5 Flash' },
    { id: 'gemini-2.5-flash-lite', name: 'Gemini 2.5 Flash-Lite' },
    { id: 'gemini-3-pro-preview', name: 'Gemini 3 Pro (Preview)' },
    { id: 'gemini-3-flash-preview', name: 'Gemini 3 Flash (Preview)' },
  ],
  copilot: [
    { id: '', name: 'Default' },
  ],
};

// Thinking level options (codex uses model_reasoning_effort config)
const THINKING_LEVELS: Record<string, { id: string; name: string }[]> = {
  codex: [
    { id: '', name: 'Default' },
    { id: 'low', name: 'Low' },
    { id: 'medium', name: 'Medium' },
    { id: 'high', name: 'High' },
    { id: 'xhigh', name: 'XHigh' },
  ],
};

export default function Settings() {
  const { t, i18n } = useTranslation();
  const { settings, loadSettings, saveSettings } = useAppStore();
  const [selectedCli, setSelectedCli] = useState(settings.ai_cli);
  const [selectedModel, setSelectedModel] = useState(settings.ai_model || '');
  const [selectedThinking, setSelectedThinking] = useState(settings.ai_thinking || '');
  const [hardcodeEnabled, setHardcodeEnabled] = useState(settings.hardcode_enabled);
  const [suspectedEnabled, setSuspectedEnabled] = useState(settings.suspected_enabled);
  const [scanScope, setScanScope] = useState(settings.scan_scope);
  const [saved, setSaved] = useState(false);
  const [cliStatuses, setCliStatuses] = useState<Record<string, CliStatus>>({});

  useEffect(() => {
    loadSettings();
  }, [loadSettings]);

  useEffect(() => {
    let active = true;
    api.detectAiClis()
      .then((statuses) => {
        if (active) setCliStatuses(statuses);
      })
      .catch(() => {
        if (active) setCliStatuses(EMPTY_CLI_STATUSES);
      });
    return () => {
      active = false;
    };
  }, []);

  // Sync local state when settings load from backend
  useEffect(() => {
    setSelectedCli(settings.ai_cli);
    setSelectedModel(settings.ai_model || '');
    setSelectedThinking(settings.ai_thinking || '');
    setHardcodeEnabled(settings.hardcode_enabled);
    setSuspectedEnabled(settings.suspected_enabled);
    setScanScope(settings.scan_scope);
    if (settings.language && settings.language !== i18n.language) {
      i18n.changeLanguage(settings.language);
    }
  }, [settings]);

  const handleSave = async () => {
    await saveSettings({
      ai_cli: selectedCli,
      language: i18n.language,
      scan_scope: scanScope,
      hardcode_enabled: hardcodeEnabled,
      suspected_enabled: suspectedEnabled,
      ai_model: selectedModel || null,
      ai_thinking: selectedThinking || null,
    });
    setSaved(true);
    setTimeout(() => setSaved(false), 2000);
  };

  const handleReset = () => {
    setSelectedCli('claude');
    setSelectedModel('');
    setSelectedThinking('');
    setHardcodeEnabled(true);
    setSuspectedEnabled(true);
    setScanScope('full');
  };

  return (
    <div className="page-main">
      <div className="page-header">
        <div className="page-title">{t('settings.title')}</div>
      </div>
      <p style={{ color: 'var(--text-dimmer)', fontSize: 13, marginBottom: 32 }}>
        {t('settings.desc')}
      </p>

      {/* AI CLI */}
      <div className="settings-section">
        <div className="settings-label">{t('settings.aiCli')}</div>
        <div className="settings-desc">{t('settings.selectCli')}</div>
        <div className="cli-grid">
          {CLI_OPTIONS.map((cli) => {
            const status = cliStatuses[cli.id];
            const isAvailable = status?.available === true;
            const statusText = status
              ? (isAvailable ? `✓ ${t('settings.detected')}` : t('settings.notFound'))
              : t('common.loading');

            return (
              <div
                key={cli.id}
                className={`cli-card${selectedCli === cli.id ? ' selected' : ''}`}
                onClick={() => { setSelectedCli(cli.id); setSelectedModel(''); setSelectedThinking(''); }}
                title={status?.resolved_path || cli.name}
              >
                <div style={{ fontSize: 28, marginBottom: 8 }}>{cli.icon}</div>
                <div className="cli-name">{cli.name}</div>
                <div className={`cli-status ${isAvailable ? 'cli-detected' : 'cli-missing'}`}>
                  {statusText}
                </div>
              </div>
            );
          })}
        </div>

        {/* Model selector */}
        {CLI_MODELS[selectedCli] && CLI_MODELS[selectedCli].length > 1 && (
          <div style={{ marginTop: 16 }}>
            <div className="settings-desc">{t('settings.modelDesc')}</div>
            <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap', marginTop: 8 }}>
              {CLI_MODELS[selectedCli].map((m) => (
                <button
                  key={m.id}
                  className={`btn ${selectedModel === m.id ? 'btn-primary' : 'btn-ghost'}`}
                  style={{ padding: '4px 12px', fontSize: 12 }}
                  onClick={() => setSelectedModel(m.id)}
                >
                  {m.name}
                </button>
              ))}
            </div>
          </div>
        )}

        {/* Thinking level selector */}
        {THINKING_LEVELS[selectedCli] && (
          <div style={{ marginTop: 16 }}>
            <div className="settings-desc">{t('settings.thinkingDesc')}</div>
            <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap', marginTop: 8 }}>
              {THINKING_LEVELS[selectedCli].map((l) => (
                <button
                  key={l.id}
                  className={`btn ${selectedThinking === l.id ? 'btn-primary' : 'btn-ghost'}`}
                  style={{ padding: '4px 12px', fontSize: 12 }}
                  onClick={() => setSelectedThinking(l.id)}
                >
                  {l.name}
                </button>
              ))}
            </div>
          </div>
        )}
      </div>

      {/* Language */}
      <div className="settings-section">
        <div className="settings-label">{t('settings.language')}</div>
        <div className="settings-desc">{t('settings.langDesc')}</div>
        <div style={{ display: 'flex', gap: 12 }}>
          <button
            className={`btn ${i18n.language === 'zh' ? 'btn-primary' : 'btn-ghost'}`}
            onClick={() => { i18n.changeLanguage('zh'); }}
          >
            中文
          </button>
          <button
            className={`btn ${i18n.language === 'en' ? 'btn-primary' : 'btn-ghost'}`}
            onClick={() => { i18n.changeLanguage('en'); }}
          >
            English
          </button>
        </div>
      </div>

      {/* Analysis params */}
      <div className="settings-section">
        <div className="settings-label">{t('settings.analysis')}</div>
        <div className="settings-desc">{t('settings.scanScopeDesc')}</div>

        <div className="setting-toggle">
          <div>
            <div className="toggle-label">{t('settings.scanScope')}</div>
            <div className="toggle-desc">{t('settings.scanScopeDesc')}</div>
          </div>
          <div style={{ display: 'flex', gap: 8 }}>
            <button
              className={`btn ${scanScope === 'full' ? 'btn-primary' : 'btn-ghost'}`}
              style={{ padding: '4px 12px', fontSize: 12 }}
              onClick={() => setScanScope('full')}
            >
              {t('settings.fullProject')}
            </button>
            <button
              className={`btn ${scanScope === 'custom' ? 'btn-primary' : 'btn-ghost'}`}
              style={{ padding: '4px 12px', fontSize: 12 }}
              onClick={() => setScanScope('custom')}
            >
              {t('settings.customScope')}
            </button>
          </div>
        </div>

        <div className="setting-toggle">
          <div>
            <div className="toggle-label">{t('settings.hardcodeDetection')}</div>
            <div className="toggle-desc">{t('settings.hardcodeDetectionDesc')}</div>
          </div>
          <button
            className={`toggle-switch${hardcodeEnabled ? ' on' : ''}`}
            onClick={() => setHardcodeEnabled(!hardcodeEnabled)}
          />
        </div>

        <div className="setting-toggle">
          <div>
            <div className="toggle-label">{t('settings.suspectedDetection')}</div>
            <div className="toggle-desc">{t('settings.suspectedDetectionDesc')}</div>
          </div>
          <button
            className={`toggle-switch${suspectedEnabled ? ' on' : ''}`}
            onClick={() => setSuspectedEnabled(!suspectedEnabled)}
          />
        </div>
      </div>

      <div style={{ display: 'flex', gap: 12, marginTop: 24, alignItems: 'center' }}>
        <button className="btn btn-primary" onClick={handleSave}>{t('settings.save')}</button>
        <button className="btn btn-ghost" onClick={handleReset}>{t('settings.reset')}</button>
        {saved && <span style={{ color: 'var(--accent)', fontSize: 13 }}>✓ {t('settings.saved') || '已保存'}</span>}
      </div>
    </div>
  );
}
