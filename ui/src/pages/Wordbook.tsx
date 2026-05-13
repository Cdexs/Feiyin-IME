import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { getTranslations } from '../i18n';

interface Entry {
  id: number;
  raw: string;
  corrected: string;
  source: 'system' | 'user';
}

interface Props {
  config: any;
  updateConfig: (cfg: any) => void;
}

interface ErrorDialogState {
  show: boolean;
  title: string;
  message: string;
}

const closeErrorDialogState: ErrorDialogState = {
  show: false,
  title: '',
  message: '',
};

const sanitizeDialogMessage = (message: string, fallback: string) => {
  const trimmed = message.trim();
  if (!trimmed) {
    return fallback;
  }
  return trimmed
    .replace(/^tauri\.localhost\s*(显示|says)?\s*[:：-]?\s*/i, '')
    .trim() || fallback;
};

const WordbookPage: React.FC<Props> = ({ config }) => {
  const t = getTranslations(config.ui_language);
  const [activeTab, setActiveTab] = useState<'system' | 'user'>('system');
  const [entries, setEntries] = useState<Entry[]>([]);
  const [loading, setLoading] = useState(true);
  const [showAddModal, setShowAddModal] = useState(false);
  const [newRaw, setNewRaw] = useState('');
  const [newCorrected, setNewCorrected] = useState('');
  const [errorDialog, setErrorDialog] = useState<ErrorDialogState>(closeErrorDialogState);

  useEffect(() => {
    loadEntries();
  }, []);

  const loadEntries = async () => {
    try {
      const data = await invoke<Entry[]>('get_wordbook_entries');
      setEntries(data);
    } catch (e) {
      console.error('Failed to load wordbook entries:', e);
    } finally {
      setLoading(false);
    }
  };

  const filteredEntries = entries.filter(e => e.source === activeTab);
  const systemCount = entries.filter(e => e.source === 'system').length;
  const userCount = entries.filter(e => e.source === 'user').length;
  const total = systemCount + userCount;

  const handleDelete = async (id: number) => {
    const entry = entries.find(e => e.id === id);
    if (!entry) return;

    try {
      await invoke('delete_wordbook_entry_by_id', { id });
      setEntries(prev => prev.filter(e => e.id !== id));
    } catch (e) {
      setErrorDialog({
        show: true,
        title: '',
        message: t.wordbook_delete_failed,
      });
    }
  };

  const handleAdd = async () => {
    if (!newRaw.trim() || !newCorrected.trim()) return;

    try {
      await invoke('add_wordbook_entry', { raw: newRaw.trim(), corrected: newCorrected.trim() });
      await loadEntries();
      setNewRaw('');
      setNewCorrected('');
      setShowAddModal(false);
    } catch (e) {
      setErrorDialog({
        show: true,
        title: t.wordbook_add_failed,
        message: sanitizeDialogMessage(
          e instanceof Error ? e.message : String(e ?? ''),
          t.wordbook_add_failed_fallback
        ),
      });
    }
  };

  return (
    <div className="settings-page">
      <h2 className="page-title">{t.wordbook_title}</h2>

      <div className="wordbook-stats">
        <span className="wordbook-stat-total">
          {t.wordbook_total_prefix}
          <span className="wordbook-stat-num">{total}</span>
          {t.wordbook_total_suffix}
        </span>
        <span className="wordbook-stat-divider" />
        <span className="wordbook-stat">
          {t.wordbook_system + ' ' + systemCount}
        </span>
        <span className="wordbook-stat-divider" />
        <span className="wordbook-stat">
          {t.wordbook_user + ' ' + userCount}
        </span>
      </div>

      <div className="wordbook-tabs">
        <button
          className={`wordbook-tab ${activeTab === 'system' ? 'active' : ''}`}
          onClick={() => setActiveTab('system')}
        >
          {t.wordbook_system}
        </button>
        <button
          className={`wordbook-tab ${activeTab === 'user' ? 'active' : ''}`}
          onClick={() => setActiveTab('user')}
        >
          {t.wordbook_user}
        </button>
        <button
          className={`wordbook-add-inline ${activeTab !== 'user' ? 'disabled' : ''}`}
          onClick={() => activeTab === 'user' && setShowAddModal(true)}
          disabled={activeTab !== 'user'}
          title={t.wordbook_add_entry}
        >
          +
        </button>
      </div>

      <div className="wordbook-content">
        {loading ? (
          <div className="wordbook-loading">{t.wordbook_loading}</div>
        ) : (
          <div className="wordbook-labels">
            {filteredEntries.map(entry => (
              <span className="wordbook-label" key={entry.id}>
                <span className="wordbook-label-text">{entry.corrected}</span>
                <button
                  className="wordbook-label-delete"
                  onClick={() => handleDelete(entry.id)}
                  title={t.wordbook_delete}
                >
                  ×
                </button>
              </span>
            ))}
          </div>
        )}
      </div>

      {showAddModal && (
        <div className="modal-overlay">
          <div className="modal-dialog modal-wordbook-add" style={{ maxWidth: '420px' }} role="dialog" onClick={e => e.stopPropagation()}>
            <div className="modal-header">
              <svg className="modal-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <line x1="8" y1="6" x2="21" y2="6" />
                <line x1="8" y1="12" x2="21" y2="12" />
                <line x1="8" y1="18" x2="21" y2="18" />
                <line x1="3" y1="6" x2="3.01" y2="6" />
                <line x1="3" y1="12" x2="3.01" y2="12" />
                <line x1="3" y1="18" x2="3.01" y2="18" />
              </svg>
              <span className="modal-title">{t.wordbook_add_entry}</span>
              <button className="modal-close" onClick={() => setShowAddModal(false)}>×</button>
            </div>
            <div className="modal-body">
              <div className="form-group">
                <label className="form-label">{t.wordbook_original}</label>
                <input
                  className="input"
                  type="text"
                  value={newRaw}
                  onChange={e => setNewRaw(e.target.value)}
                  placeholder={t.wordbook_original_placeholder}
                  autoFocus
                />
              </div>
              <div className="form-group">
                <label className="form-label">{t.wordbook_replacement}</label>
                <input
                  className="input"
                  type="text"
                  value={newCorrected}
                  onChange={e => setNewCorrected(e.target.value)}
                  placeholder={t.wordbook_replacement_placeholder}
                />
              </div>
              <div className="modal-hint">
                <span className="modal-hint-icon">ⓘ</span>
                <span>{t.wordbook_add_hint}</span>
              </div>
            </div>
            <div className="modal-footer">
              <button className="btn btn-secondary" onClick={() => setShowAddModal(false)}>{t.wordbook_cancel}</button>
              <button className="btn btn-primary btn-accent" onClick={handleAdd} disabled={!newRaw.trim() || !newCorrected.trim()}>{t.wordbook_add}</button>
            </div>
          </div>
        </div>
      )}

      {errorDialog.show && (
        <div className="modal-overlay" onClick={() => setErrorDialog(closeErrorDialogState)}>
          <div className="modal-dialog modal-error" role="dialog" onClick={e => e.stopPropagation()}>
            {errorDialog.title && <h3 className="modal-title">{errorDialog.title}</h3>}
            <div className="modal-body">
              <p>{errorDialog.message}</p>
            </div>
            <div className="modal-footer">
              <button className="btn btn-primary btn-accent" onClick={() => setErrorDialog(closeErrorDialogState)}>{t.wordbook_ok}</button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};

export default WordbookPage;
