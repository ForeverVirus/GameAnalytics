import { NavLink } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../store';

export default function TopBar() {
  const { t, i18n } = useTranslation();
  const project = useAppStore((s) => s.project);

  const toggleLang = () => {
    i18n.changeLanguage(i18n.language === 'zh' ? 'en' : 'zh');
  };

  const navItems = [
    { to: '/overview', label: t('nav.overview') },
    { to: '/performance', label: t('nav.performance', '性能分析') },
    { to: '/redundancy', label: t('nav.redundancy', '冗余资源') },
    { to: '/asset', label: t('nav.asset') },
    { to: '/code', label: t('nav.code') },
    { to: '/suspected', label: t('nav.suspected') },
    { to: '/hardcode', label: t('nav.hardcode') },
    { to: '/settings', label: t('nav.settings') },
  ];

  return (
    <div className="topbar">
      <NavLink to="/overview" className="logo">
        ⬡ CodeGraph <span className="logo-sub">{t('appName')}</span>
      </NavLink>
      <nav className="nav">
        {navItems.map((item) => (
          <NavLink
            key={item.to}
            to={item.to}
            className={({ isActive }) => `nav-item${isActive ? ' active' : ''}`}
          >
            {item.label}
          </NavLink>
        ))}
      </nav>
      <div className="right-actions">
        {project && (
          <span className="engine-badge">{project.engine}</span>
        )}
        <button className="lang-switch" onClick={toggleLang}>
          {i18n.language === 'zh' ? 'EN' : '中文'}
        </button>
      </div>
    </div>
  );
}
