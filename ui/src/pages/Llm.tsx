import React, { useState } from 'react';
import { invoke } from "@tauri-apps/api/core";
import { getTranslations } from '../i18n';

interface Props {
  config: any;
  updateConfig: (cfg: any) => void;
}

const LlmPage: React.FC<Props> = ({ config, updateConfig }) => {
  const [testResult, setTestResult] = useState<{ msg: string; success: boolean } | null>(null);
  const [testing, setTesting] = useState(false);
  const t = getTranslations(config.ui_language);

  const handleLlmChange = (field: string, value: any) => {
    updateConfig({
      ...config,
      llm: { ...config.llm, [field]: value }
    });
  };

  const handleTest = async () => {
    setTesting(true);
    setTestResult(null);
    try {
      const res = await invoke<string>("test_llm_connection", { config: config.llm });
      setTestResult({ msg: `Success: ${res}`, success: true });
      handleLlmChange('connectivity_verified', true);
    } catch (e) {
      setTestResult({ msg: `Failed: ${e}`, success: false });
      handleLlmChange('connectivity_verified', false);
    } finally {
      setTesting(false);
    }
  };

  return (
    <div className="settings-page">
      <h2 className="page-title">{t.llm_title}</h2>

      <section className="settings-section">
        <label className="toggle-switch card" style={{ border: 'none', boxShadow: 'none', padding: '8px 0', marginBottom: 0 }}>
          <input
            type="checkbox"
            checked={config.llm.enabled}
            onChange={(e) => handleLlmChange('enabled', e.target.checked)}
            className="toggle-input"
          />
          <span className="toggle-track"></span>
          <span className="toggle-label" style={{ fontWeight: 600 }}>{t.llm_enable}</span>
        </label>
        <p className="form-hint" style={{ color: '#FF6B35', marginTop: '4px', paddingLeft: '8px' }}>{t.llm_enable_hint}</p>
      </section>

      <section className="settings-section">
        <h3 className="section-title">{t.llm_api_config}</h3>
        <div className="card">
          <div style={{ display: 'flex', gap: '16px', marginBottom: '12px' }}>
            <div className="form-row" style={{ flex: 2, marginBottom: 0 }}>
              <label className="form-label">{t.llm_api_url}</label>
              <div className="form-control" style={{ maxWidth: '100%' }}>
                <input
                  type="text"
                  value={config.llm.api_url}
                  onChange={(e) => handleLlmChange('api_url', e.target.value)}
                  className="input"
                  placeholder="https://api.openai.com/v1"
                />
              </div>
            </div>
            <div className="form-row" style={{ flex: 1, marginBottom: 0 }}>
              <label className="form-label" style={{ minWidth: 'auto' }}>{t.llm_api_key}</label>
              <div className="form-control" style={{ maxWidth: '100%' }}>
                <input
                  type="password"
                  value={config.llm.api_key}
                  onChange={(e) => handleLlmChange('api_key', e.target.value)}
                  className="input"
                  placeholder="sk-..."
                />
              </div>
            </div>
          </div>

          <div className="form-row">
            <label className="form-label">{t.llm_model}</label>
            <div className="form-control">
              <input
                type="text"
                value={config.llm.model}
                onChange={(e) => handleLlmChange('model', e.target.value)}
                className="input"
                style={{ maxWidth: '200px' }}
              />
            </div>
          </div>

          <div className="form-row" style={{ marginTop: '20px' }}>
            <label className="form-label"></label>
            <div className="form-control">
              <button
                onClick={handleTest}
                disabled={testing}
                className="btn btn-primary"
                style={{ width: '100%', maxWidth: '200px', display: 'flex', alignItems: 'center', justifyContent: 'center', gap: '8px' }}
              >
                <span>{testing ? t.llm_testing : t.llm_test_connection}</span>
                <span
                  style={{
                    display: "inline-block",
                    width: "10px",
                    height: "10px",
                    borderRadius: "2px",
                    backgroundColor:
                      config.llm.connectivity_verified === true
                        ? "#10b981"
                        : config.llm.connectivity_verified === false
                        ? "#ef4444"
                        : "#9ca3af",
                    flexShrink: 0,
                    alignSelf: "center",
                  }}
                />
              </button>

              {testResult && (
                <div
                  className={`badge ${testResult.success ? 'badge-success' : 'badge-error'}`}
                  style={{ marginTop: '12px', padding: '10px', wordBreak: 'break-all', borderRadius: '6px', width: '100%', boxSizing: 'border-box' }}
                >
                  {testResult.msg}
                </div>
              )}
            </div>
          </div>
        </div>
      </section>
    </div>
  );
};

export default LlmPage;
