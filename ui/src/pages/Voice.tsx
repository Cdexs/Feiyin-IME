import React, { useState, useEffect, useRef } from 'react';
import { invoke } from "@tauri-apps/api/core";
import { getTranslations } from '../i18n';
import ShortcutSetting from '../components/ShortcutSetting';
import FeiyinServerPairing from '../components/FeiyinServerPairing';

interface Props {
  config: any;
  updateConfig: (cfg: any) => void;
}

const VoicePage: React.FC<Props> = ({ config, updateConfig }) => {
  const [devices, setDevices] = useState<string[]>([]);
  const t = getTranslations(config.ui_language);

  const [qwen3TestStatus, setQwen3TestStatus] = useState<'idle' | 'testing' | 'success' | 'failed'>('idle');
  const [qwen3TestMessage, setQwen3TestMessage] = useState('');
  const prevModelRef = useRef<string>('feiyin_server');
  const latestRef = useRef({ asrModel: '', config: {} as any });
  // ASR-SERVER-491 S8：识别服务是否已配对（没配对就离开本页 ⇒ 同无 key 一样回落上一个档位）。
  const remotePairedRef = useRef(false);
  // FLASH-P3-A（DEC-114）：输入法不再带本地模型，模型下拉只剩两项 ——「连识别服务」（flashasr，新装默认）
  // 与「直连阿里 FunASR」。本地档（快速 / 本地流式实时）、在线 Qwen Audio 的选项、描述与模型下载提示整体删除；
  // 旧配置值由主程序 / 设置界面加载时迁移为 feiyin_server，缺值回落同新装默认。
  const asrModel = config.audio?.asr_model ?? "feiyin_server";

  useEffect(() => {
    loadDevices();
  }, []);

  // Keep latest values in ref for cleanup closure
  useEffect(() => {
    latestRef.current = { asrModel, config };
  });

  useEffect(() => {
    return () => {
      const { asrModel: lastModel, config: lastConfig } = latestRef.current;
      // 直连阿里无 key / 连识别服务未配对就离开本页 ⇒ 回落到 prevModel
      // （回落目标与当前相同 ⇒ 不写，免得新装默认「连识别服务」未配对时每次离开都白存一次）
      if (prevModelRef.current !== lastModel
          && ((lastModel === 'fun_asr_realtime' && !lastConfig.audio?.asr_online_api_key)
            || (lastModel === 'feiyin_server' && !remotePairedRef.current))) {
        updateConfig({
          ...lastConfig,
          audio: { ...lastConfig.audio, asr_model: prevModelRef.current }
        });
      }
    };
  }, []);

const loadDevices = async () => {
    try {
      const devList = await invoke<string[]>("get_audio_devices");
      setDevices(devList);
    } catch (e) {
      console.error("Failed to load devices:", e);
    }
  };

  const handleAudioChange = (field: string, value: any) => {
    updateConfig({
      ...config,
      audio: { ...config.audio, [field]: value }
    });
  };

  const handleAsrModelChange = (value: string) => {
    // ASR-056: 切换档位时记录 prevModel（cleanup 回落用）
    if ((value === 'fun_asr_realtime' || value === 'feiyin_server') && asrModel !== value) {
      prevModelRef.current = asrModel;
    }
    if (value === 'fun_asr_realtime' && !config.audio?.asr_online_api_key) {
      setQwen3TestStatus('idle');
    }
    // ASR-056: 切到直连阿里时同步 asr_online_model 为 fun-asr 默认串。
    // 前缀守卫在主程序 load 时已兜底，这里主动同步避免守卫 warn 噪音。
    //
    // 🔴 必须一次 updateConfig 写两个字段（FUN-UI-004，tester-1 阶段三查出）：
    // handleAudioChange 每次都从**本次渲染捕获的** config 展开，连续调用两次时
    // 第二次会用同一份陈旧 config 覆盖第一次的结果 —— asr_online_model 的同步
    // 直接丢失。改为单次 patch 提交。
    const familyModelId = value === 'fun_asr_realtime' ? 'fun-asr-realtime' : null;
    updateConfig({
      ...config,
      audio: {
        ...config.audio,
        asr_model: value,
        ...(familyModelId ? { asr_online_model: familyModelId } : {})
      }
    });
  };

  const handleTestQwen3Connection = async () => {
    setQwen3TestStatus('testing');
    setQwen3TestMessage('');
    try {
      const result = await invoke<string>("test_qwen3_asr_connection", { apiKey: config.audio?.asr_online_api_key || '' });
      setQwen3TestStatus('success');
      setQwen3TestMessage(result);
    } catch (e: any) {
      setQwen3TestStatus('failed');
      setQwen3TestMessage(typeof e === 'string' ? e : e?.message || String(e));
    }
  };

  const getAsrDesc = () => {
    switch (asrModel) {
      case 'fun_asr_realtime': return t.voice_asr_model_fun_asr_desc;
      case 'feiyin_server': return t.voice_asr_model_feiyin_desc;
      default: return '';
    }
  };

  return (
    <div className="settings-page">
      <h2 className="page-title">{t.voice_title}</h2>

      <section className="settings-section">
        <h3 className="section-title">{t.voice_input_device}</h3>
        <div className="card">
          <p className="form-hint" style={{ marginBottom: '12px' }}>{t.voice_input_device_hint}</p>
          <select
            value={config.audio.input_device}
            onChange={(e) => handleAudioChange('input_device', e.target.value)}
            className="select-input"
          >
            <option value="">{t.voice_default_device}</option>
            {devices?.map(d => <option key={d} value={d}>{d}</option>)}
          </select>
        </div>
      </section>

      <section className="settings-section">
        <h3 className="section-title">{t.voice_asr_model}</h3>
        <div className="card">
          <select
            value={asrModel}
            onChange={(e) => handleAsrModelChange(e.target.value)}
            className="select-input"
          >
            <option value="feiyin_server">{t.voice_asr_model_feiyin}</option>
            <option value="fun_asr_realtime">{t.voice_asr_model_fun_asr}</option>
          </select>

          <p className="asr-model-desc" style={{ marginTop: '8px' }}>
            {getAsrDesc()}
          </p>

          {asrModel === 'feiyin_server' && (
            <FeiyinServerPairing
              t={t}
              onPairedChange={(paired) => { remotePairedRef.current = paired; }}
            />
          )}

          {asrModel === 'fun_asr_realtime' && (
            <div className="qwen3-section" style={{ marginTop: '16px' }}>
              {!config.audio?.asr_online_api_key && (
                <p className="form-hint" style={{ color: 'var(--brand-primary)', marginBottom: '8px' }}>
                  {t.voice_qwen3_empty_key_hint}
                </p>
              )}
              <div className="form-group">
                <span className="form-label">{t.voice_asr_online_api_key}</span>
                {/* UI-ASRKEY-225: 提示文字用**原生 placeholder**（不是 value、不是 focus/blur 事件填值）——
                    浏览器只在 value === '' 时渲染它，输入即消失、清空即回来，
                    🔴 且它永远不进 value/state/config.toml（Gavin 一票否决判据）。
                    样式（斜体 + 淡灰）由 .asr-key-input::placeholder 限定在本输入框，不污染其它输入框。 */}
                <input
                  type="password"
                  value={config.audio?.asr_online_api_key || ''}
                  onChange={(e) => handleAudioChange('asr_online_api_key', e.target.value)}
                  className="input asr-key-input"
                  placeholder={t.voice_asr_online_api_key_placeholder}
                />
              </div>
              <div className="form-group" style={{ marginTop: '12px' }}>
                <button
                  type="button"
                  onClick={handleTestQwen3Connection}
                  className="btn btn-primary btn-sm"
                  disabled={qwen3TestStatus === 'testing' || !config.audio?.asr_online_api_key}
                >
                  {qwen3TestStatus === 'testing' ? t.voice_qwen3_testing : t.voice_qwen3_test_connection}
                </button>
                {qwen3TestStatus === 'success' && (
                  <span className="qwen3-test-result qwen3-test-success">
                    {t.voice_qwen3_test_success}
                  </span>
                )}
                {qwen3TestStatus === 'failed' && (
                  <span className="qwen3-test-result qwen3-test-failed">
                    {t.voice_qwen3_test_failed}
                  </span>
                )}
                {qwen3TestMessage && (
                  <p className="form-hint" style={{ marginTop: '4px', wordBreak: 'break-all' }}>
                    {qwen3TestMessage}
                  </p>
                )}
              </div>
            </div>
          )}
        </div>
      </section>

      <section className="settings-section">
        <h3 className="section-title">{t.voice_recognition_output}</h3>
        <div className="card">
          <label className="toggle-switch" style={{ border: 'none', boxShadow: 'none', padding: '8px 0', marginBottom: '4px' }}>
            <input
              type="checkbox"
              checked={config.punctuation?.enabled ?? true}
              onChange={(e) => updateConfig({
                ...config,
                punctuation: { ...config.punctuation, enabled: e.target.checked }
              })}
              className="toggle-input"
            />
            <span className="toggle-track"></span>
            <span className="toggle-label">{t.voice_auto_punctuation}</span>
          </label>
          {/* V091-PUNCT-TAIL-214: 句尾不显示标点符号（默认关，绑 punctuation.strip_trailing） */}
          {/* SC-PUNCT-486：后接快捷方式链接（在 label 外，点链接不拨动开关） */}
          {/* SC-ALIGN-490：行距放在行容器上，label 不带 margin，否则标签文字比快捷方式高 2px */}
          <div className="toggle-row" style={{ marginBottom: '4px' }}>
            <label className="toggle-switch" style={{ border: 'none', boxShadow: 'none', padding: '8px 0', marginBottom: 0 }}>
              <input
                type="checkbox"
                checked={config.punctuation?.strip_trailing ?? false}
                onChange={(e) => updateConfig({
                  ...config,
                  punctuation: { ...config.punctuation, strip_trailing: e.target.checked }
                })}
                className="toggle-input"
              />
              <span className="toggle-track"></span>
              <span className="toggle-label">{t.voice_strip_trailing_punct}</span>
            </label>
            <ShortcutSetting config={config} updateConfig={updateConfig} action="strip_trailing_toggle" />
          </div>
          <p className="form-hint" style={{ marginBottom: '12px' }}>{t.voice_strip_trailing_punct_hint}</p>
          <div className="form-group" style={{ marginTop: '16px' }}>
            {/* SC-SCRIPT-489：「中文输出」后接快捷方式（按下在简体 / 繁体之间切换） */}
            <div className="toggle-row" style={{ marginBottom: '4px' }}>
              <span className="form-label">{t.voice_chinese_output}</span>
              <ShortcutSetting config={config} updateConfig={updateConfig} action="chinese_script_toggle" />
            </div>
            <div className="radio-group" style={{ marginTop: '8px' }}>
              <label className={`radio-card ${config.audio.chinese_script === 'Simplified' ? 'active' : ''}`}>
                <input
                  type="radio"
                  name="chinese_script"
                  checked={config.audio.chinese_script === 'Simplified'}
                  onChange={() => handleAudioChange('chinese_script', 'Simplified')}
                  className="radio-input"
                />
                <span className="custom-radio"></span>
                <div className="radio-content">
                  <span className="radio-title">{t.voice_simplified}</span>
                </div>
              </label>
              <label className={`radio-card ${config.audio.chinese_script === 'Traditional' ? 'active' : ''}`}>
                <input
                  type="radio"
                  name="chinese_script"
                  checked={config.audio.chinese_script === 'Traditional'}
                  onChange={() => handleAudioChange('chinese_script', 'Traditional')}
                  className="radio-input"
                />
                <span className="custom-radio"></span>
                <div className="radio-content">
                  <span className="radio-title">{t.voice_traditional}</span>
                </div>
              </label>
            </div>
          </div>
        </div>
      </section>
    </div>
  );
};

export default VoicePage;
