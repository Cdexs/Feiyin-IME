import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { getTranslations } from "./i18n";
import "./styles.css";
import GeneralPage from "./pages/General";
import HotkeySettingsPage from "./pages/HotkeySettings";
import VoicePage from "./pages/Voice";
import LlmPage from "./pages/Llm";
import WordbookPage from "./pages/Wordbook";
import AboutPage from "./pages/About";

function App() {
  const appWindow = getCurrentWindow();
  const [activeTab, setActiveTab] = useState("general");
  const [config, setConfig] = useState<any>(null);
  const [loading, setLoading] = useState(true);
  const [showPromptModal, setShowPromptModal] = useState(false);

  useEffect(() => {
    const disableMaximize = async () => {
      try {
        await appWindow.setMaximizable(false);
      } catch (e) {
        console.error("Failed to disable maximize button:", e);
      }
    };
    disableMaximize();
  }, [appWindow]);

  useEffect(() => {
    loadConfig();
  }, []);

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.ctrlKey && e.key === "t") {
        e.preventDefault();
        setShowPromptModal(prev => !prev);
      }
      if (e.key === "Escape" && showPromptModal) {
        setShowPromptModal(false);
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [showPromptModal]);

  const loadConfig = async () => {
    try {
      const cfg = await invoke("get_config");
      setConfig(cfg);
    } catch (e) {
      console.error("Failed to load config:", e);
    } finally {
      setLoading(false);
    }
  };

  const updateConfig = async (newConfig: any) => {
    setConfig(newConfig);
    try {
      await invoke("save_config", { config: newConfig });
    } catch (e) {
      console.error("Failed to save config:", e);
    }
  };

  if (loading) return <div className="loading-state">Loading...</div>;
  if (!config) return <div className="empty-state">Error: Could not load configuration.</div>;

  const t = getTranslations(config.ui_language);

  const renderTab = () => {
    const props = { config, updateConfig };
    switch (activeTab) {
      case "general": return <GeneralPage {...props} />;
      case "hotkey": return <HotkeySettingsPage {...props} />;
      case "voice": return <VoicePage {...props} />;
      case "llm": return <LlmPage {...props} />;
      case "wordbook": return <WordbookPage {...props} />;
      case "about": return <AboutPage {...props} />;
      default: return <GeneralPage {...props} />;
    }
  };

  const navItems = [
    { id: 'general', label: t.nav_general },
    { id: 'hotkey', label: t.nav_hotkey },
    { id: 'voice', label: t.nav_voice },
    { id: 'llm', label: t.nav_llm },
    { id: 'wordbook', label: t.nav_wordbook },
    { id: 'about', label: t.nav_about },
  ];

  const navIcons: { [key: string]: JSX.Element } = {
    general: (
      <svg className="sidebar-nav-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
        <path d="M12 15a3 3 0 100-6 3 3 0 000 6z"/><path d="M19.4 15a1.65 1.65 0 00.33 1.82l.06.06a2 2 0 01-2.83 2.83l-.06-.06a1.65 1.65 0 00-1.82-.33 1.65 1.65 0 00-1 1.51V21a2 2 0 01-4 0v-.09A1.65 1.65 0 009 19.4a1.65 1.65 0 00-1.82.33l-.06.06a2 2 0 01-2.83-2.83l.06-.06A1.65 1.65 0 004.68 15a1.65 1.65 0 00-1.51-1H3a2 2 0 010-4h.09A1.65 1.65 0 004.6 9a1.65 1.65 0 00-.33-1.82l-.06-.06a2 2 0 012.83-2.83l.06.06A1.65 1.65 0 009 4.68a1.65 1.65 0 001-1.51V3a2 2 0 014 0v.09c.04.56.34 1.06.78 1.38.14.1.28.18.43.24A1.65 1.65 0 0015 4.68a1.65 1.65 0 001.51 1H21a2 2 0 010 4h-.09a1.65 1.65 0 00-1.38.78 1.65 1.65 0 00-.24.43c-.1.14-.18.28-.24.43A1.65 1.65 0 0019.4 9a1.65 1.65 0 001 1.51H21a2 2 0 010 4h-.09c-.56.04-1.06.34-1.38.78-.1.14-.18.28-.24.43z"/>
      </svg>
    ),
    hotkey: (
      <svg className="sidebar-nav-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
        <rect x="2" y="6" width="20" height="13" rx="2"/>
        <path d="M6 10h.01M10 10h.01M14 10h.01M18 10h.01M8 14h8"/>
      </svg>
    ),
    voice: (
      <svg className="sidebar-nav-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
        <path d="M12 1a3 3 0 00-3 3v8a3 3 0 006 0V4a3 3 0 00-3-3z"/><path d="M19 10v2a7 7 0 01-14 0v-2"/><path d="M12 19v4"/><path d="M8 23h8"/>
      </svg>
    ),
    llm: (
      <svg className="sidebar-nav-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
        <path d="M21 15a2 2 0 01-2 2H7l-4 4V5a2 2 0 012-2h14a2 2 0 012 2z"/>
      </svg>
    ),
    wordbook: (
      <svg className="sidebar-nav-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
        <path d="M4 19.5A2.5 2.5 0 016.5 17H20"/><path d="M6.5 2H20v20H6.5A2.5 2.5 0 014 19.5v-15A2.5 2.5 0 016.5 2z"/>
      </svg>
    ),
    about: (
      <svg className="sidebar-nav-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
        <circle cx="12" cy="12" r="10"/><path d="M12 16v-4"/><path d="M12 8h.01"/>
      </svg>
    ),
  };

  return (
    <div className="app-container">
      <div className="sidebar">
        <div className="sidebar-title">
          <div className="sidebar-title-content">
            <svg className="sidebar-logo-icon" viewBox="0 0 24 24" fill="none" stroke="var(--brand-primary)" strokeWidth="2">
              <path d="M12 1a3 3 0 00-3 3v8a3 3 0 006 0V4a3 3 0 00-3-3z"/>
              <path d="M19 10v2a7 7 0 01-14 0v-2"/>
            </svg>
            <span className="sidebar-title-text">{t.app_title}</span>
          </div>
        </div>

        <ul className="sidebar-nav">
          {navItems.map(item => (
            <li
              key={item.id}
              className={`sidebar-nav-item ${activeTab === item.id ? 'active' : ''}`}
              onClick={() => setActiveTab(item.id)}
            >
              {navIcons[item.id]}
              <span>{item.label}</span>
            </li>
          ))}
        </ul>

        <div className="sidebar-bottom">
          <button className="sidebar-settings-btn" title={t.settings_tooltip}>
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
              <path d="M12 15a3 3 0 100-6 3 3 0 000 6z"/>
              <path d="M19.4 15a1.65 1.65 0 00.33 1.82l.06.06a2 2 0 01-2.83 2.83l-.06-.06a1.65 1.65 0 00-1.82-.33 1.65 1.65 0 00-1 1.51V21a2 2 0 01-4 0v-.09A1.65 1.65 0 009 19.4a1.65 1.65 0 00-1.82.33l-.06.06a2 2 0 01-2.83-2.83l.06-.06A1.65 1.65 0 004.68 15a1.65 1.65 0 00-1.51-1H3a2 2 0 010-4h.09A1.65 1.65 0 004.6 9a1.65 1.65 0 00-.33-1.82l-.06-.06a2 2 0 012.83-2.83l.06.06A1.65 1.65 0 009 4.68a1.65 1.65 0 001-1.51V3a2 2 0 014 0v.09c.04.56.34 1.06.78 1.38.14.1.28.18.43.24A1.65 1.65 0 0015 4.68a1.65 1.65 0 001.51 1H21a2 2 0 010 4h-.09a1.65 1.65 0 00-1.38.78 1.65 1.65 0 00-.24.43c-.1.14-.18.28-.24.43A1.65 1.65 0 0019.4 9a1.65 1.65 0 001 1.51H21a2 2 0 010 4h-.09c-.56.04-1.06.34-1.38.78-.1.14-.18.28-.24.43z"/>
            </svg>
          </button>
        </div>
      </div>

      <div className="main-content">
        <div className="main-content-inner">
          {renderTab()}
        </div>
      </div>

      {showPromptModal && (
        <div className="modal-overlay" onClick={() => setShowPromptModal(false)}>
          <div className="modal-dialog" role="dialog" onClick={e => e.stopPropagation()}>
            <div className="modal-header">
              <span className="modal-title">{t.prompt_modal_title}</span>
              <button className="modal-close" onClick={() => setShowPromptModal(false)}>×</button>
            </div>
            <div className="modal-body">
              <textarea
                className="textarea"
                value={config.llm?.system_prompt || ''}
                onChange={(e) => updateConfig({
                  ...config,
                  llm: { ...config.llm, system_prompt: e.target.value }
                })}
                style={{ maxWidth: '100%', width: '100%', minHeight: '200px' }}
              />
            </div>
            <div className="modal-footer">
              <button className="btn btn-primary" onClick={() => setShowPromptModal(false)}>
                {t.prompt_modal_save}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

export default App;
