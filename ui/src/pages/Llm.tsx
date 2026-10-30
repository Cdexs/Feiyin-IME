import React, { useEffect, useState } from 'react';
import { invoke } from "@tauri-apps/api/core";
import { getTranslations } from '../i18n';
import ShortcutSetting from '../components/ShortcutSetting';

/** FLASH-P3-C：服务端大模型状态（`asr_remote_status` → flashasr `GET /v1/status` 的 `llm_configured`）。 */
type ServerLlm = 'checking' | 'configured' | 'unconfigured' | 'unknown';

/**
 * FLASH-P3-C：「启用输出优化」开关的门槛。
 * - 连 flashasr（feiyin_server）：大模型在服务端统一配置 ⇒ 不要求本机填 Key / 通过测试；
 * - 直连阿里：维持现有门槛（接口地址 / 密钥 / 模型齐全且通过连接测试）。
 * 与主程序快捷方式 `AppConfig::apply_shortcut(LlmToggle)` 同一规则。
 */
export function llmCanEnable(asrModel: string, llm: any): boolean {
  if (asrModel === 'feiyin_server') return true;
  return (llm.connectivity_verified === true) &&
    (llm.api_url || '').trim().length > 0 &&
    (llm.api_key || '').trim().length > 0 &&
    (llm.model || '').trim().length > 0;
}

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

  // FLASH-P3-C：连 flashasr 时本机 API 配置不参与（缺省档位同语音页 = 新装默认 feiyin_server）。
  const asrModel = config.audio?.asr_model ?? 'feiyin_server';
  const viaServer = asrModel === 'feiyin_server';
  const canEnable = llmCanEnable(asrModel, llm);
  const [serverLlm, setServerLlm] = useState<ServerLlm>('checking');
  // 连 flashasr 时 API 配置卡默认收起（点「展开配置」仍可编辑，供以后切回直连阿里用）。
  const [apiExpanded, setApiExpanded] = useState(false);

  useEffect(() => {
    if (!viaServer) return;
    let alive = true;
    setServerLlm('checking');
    // Promise.resolve 包一层：调用方 / 测试桩的 invoke 同步返回非 Promise 时也不崩。
    Promise.resolve()
      .then(() => invoke<{ llm_configured?: boolean } | null>('asr_remote_status'))
      .then((st) => {
        if (!alive) return;
        if (!st) setServerLlm('unknown');
        else setServerLlm(st.llm_configured ? 'configured' : 'unconfigured');
      })
      .catch(() => { if (alive) setServerLlm('unknown'); });
    return () => { alive = false; };
  }, [viaServer]);

  const gateHintText = t.llm_gate_hint;

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
      : t.llm_status_untested;

  return (
    <div className="settings-page">
      <h2 className="page-title">{t.llm_title}</h2>

      <section className="settings-section">
        {/* SC-LLM-485：快捷方式链接放在 label 外，点链接不拨动开关 */}
        <div className="toggle-row">
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
          <ShortcutSetting config={config} updateConfig={updateConfig} action="llm_toggle" />
        </div>
        {!canEnable && (
          <p className="form-hint" style={{ color: 'var(--brand-primary)', marginTop: '4px', paddingLeft: '8px' }}>{gateHintText}</p>
        )}
        {/* FLASH-P3-C：连 flashasr ⇒ 显示服务端大模型状态（未配置 ⇒ 开了也会跳过） */}
        {viaServer && (
          <div style={{ marginTop: '8px', paddingLeft: '8px' }}>
            <div
              className={`llm-status-bar ${serverLlm === 'configured' ? 'success' : serverLlm === 'unconfigured' ? 'failed' : ''}`}
            >
              <span className="llm-status-dot"></span>
              <span>
                {t.llm_server_llm}{t.asr_colon}
                {serverLlm === 'checking' ? t.llm_server_llm_checking
                  : serverLlm === 'configured' ? t.llm_server_llm_configured
                  : serverLlm === 'unconfigured' ? t.llm_server_llm_unconfigured
                  : t.llm_server_llm_unknown}
              </span>
            </div>
            {serverLlm === 'unconfigured' && (
              <p className="form-hint" style={{ color: 'var(--brand-primary)', marginTop: '4px' }}>
                {t.llm_server_llm_unconfigured_hint}
              </p>
            )}
          </div>
        )}
      </section>

      <section className="settings-section">
        <h3 className="section-title">{t.llm_api_config}</h3>
        <div className="card llm-api-card">
          {viaServer && (
            <div>
              <p className="form-hint">{t.llm_server_mode_hint}</p>
              <button type="button" className="btn btn-link" onClick={() => setApiExpanded(!apiExpanded)}>
                {apiExpanded ? t.llm_api_hide : t.llm_api_show}
              </button>
            </div>
          )}
          {(!viaServer || apiExpanded) && (<>
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
          </>)}
        </div>
      </section>
    </div>
  );
};

export default LlmPage;
