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

  const llm = config.llm || {};

  const handleLlmChange = (field: string, value: any) => {
    updateConfig({
      ...config,
      llm: { ...llm, [field]: value }
    });
  };

  // FORMAT-LLM-001-UI: any change to api_url/api_key/model resets connectivity_verified
  const handleResetVerifiedChange = (field: string, value: any) => {
    updateConfig({
      ...config,
      llm: { ...llm, [field]: value, connectivity_verified: false }
    });
  };

  const canEnable =
    (llm.connectivity_verified === true) &&
    (llm.api_url || '').trim().length > 0 &&
    (llm.api_key || '').trim().length > 0 &&
    (llm.model || '').trim().length > 0;

  const gateHintText =
    "请先完整填写接口地址、接口密钥、模型名称，并通过连接测试。";

  const handleToggleEnabled = (checked: boolean) => {
    if (checked && !canEnable) {
      return;
    }
    handleLlmChange('enabled', checked);
  };

  const handleTest = async () => {
    setTesting(true);
    setTestResult(null);
    try {
      const res = await invoke<string>("test_llm_connection", { config: llm });
      setTestResult({ msg: `${t.llm_test_success}: ${res}`, success: true });
      handleLlmChange('connectivity_verified', true);
    } catch (e: any) {
      setTestResult({ msg: `${t.llm_test_failed}: ${typeof e === 'string' ? e : e?.message || String(e)}`, success: false });
      handleLlmChange('connectivity_verified', false);
    } finally {
      setTesting(false);
    }
  };

  const statusClass =
    llm.connectivity_verified === true
      ? 'success'
      : llm.connectivity_verified === false
      ? 'failed'
      : '';

  const statusText =
    llm.connectivity_verified === true
      ? t.llm_test_success
      : llm.connectivity_verified === false
      ? t.llm_test_failed
      : '未测试';

  return (
    <div className="settings-page">
      <h2 className="page-title">{t.llm_title}</h2>

      <section className="settings-section">
        <label className="toggle-switch card" style={{ border: 'none', boxShadow: 'none', padding: '8px 0', marginBottom: 0 }}>
          <input
            type="checkbox"
            checked={llm.enabled}
            onChange={(e) => handleToggleEnabled(e.target.checked)}
            className="toggle-input"
          />
          <span className="toggle-track"></span>
          <span className="toggle-label" style={{ fontWeight: 600 }}>{t.llm_enable}</span>
        </label>
        {!canEnable && (
          <p className="form-hint" style={{ color: '#FF6B35', marginTop: '4px', paddingLeft: '8px' }}>{gateHintText}</p>
        )}
      </section>

      <section className="settings-section">
        <h3 className="section-title">{t.llm_api_config}</h3>
        <div className="card llm-api-card">
          <div className="llm-form-grid">
            <div className="llm-form-field">
              <label className="llm-form-label">{t.llm_api_url}</label>
              <input
                type="text"
                value={llm.api_url || ''}
                onChange={(e) => handleResetVerifiedChange('api_url', e.target.value)}
                className="llm-input"
                placeholder="https://api.openai.com/v1"
              />
            </div>

            <div className="llm-form-field">
              <label className="llm-form-label">{t.llm_api_key}</label>
              <input
                type="password"
                value={llm.api_key || ''}
                onChange={(e) => handleResetVerifiedChange('api_key', e.target.value)}
                className="llm-input"
                placeholder="sk-..."
              />
            </div>

            <div className="llm-form-field">
              <label className="llm-form-label">{t.llm_model}</label>
              <div className="llm-model-row">
                <input
                  type="text"
                  value={llm.model || ''}
                  onChange={(e) => handleResetVerifiedChange('model', e.target.value)}
                  className="llm-input"
                  placeholder="gpt-4o-mini"
                />
                <button
                  onClick={handleTest}
                  disabled={testing}
                  className="btn btn-primary llm-test-btn"
                >
                  {testing ? t.llm_testing : t.llm_test_connection}
                </button>
              </div>
            </div>

            <div className="llm-form-field">
              <label className="llm-form-label"></label>
              <div className={`llm-status-bar ${statusClass}`}>
                <span className="llm-status-dot"></span>
                <span>{statusText}</span>
              </div>
            </div>
          </div>

          {testResult && (
            <div className={`llm-result-badge ${testResult.success ? 'success' : 'error'}`}>
              {testResult.msg}
            </div>
          )}
        </div>
      </section>
    </div>
  );
};

export default LlmPage;
