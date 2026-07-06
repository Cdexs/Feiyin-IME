import React, { useState, useEffect } from 'react';
import { invoke } from "@tauri-apps/api/core";
import { getTranslations } from '../i18n';

interface Props {
  config: any;
  updateConfig: (cfg: any) => void;
}

interface AccuracyModelInfo {
  ready: boolean;
  model_dir: string;
  download_url: string;
}

const VoicePage: React.FC<Props> = ({ config, updateConfig }) => {
  const [devices, setDevices] = useState<string[]>([]);
  const [modelInfo, setModelInfo] = useState<AccuracyModelInfo | null>(null);
  const [copiedField, setCopiedField] = useState<'url' | 'dir' | null>(null);
  const t = getTranslations(config.ui_language);

  const asrModel = config.audio?.asr_model ?? "performance";

  useEffect(() => {
    loadDevices();
  }, []);

  useEffect(() => {
    checkAccuracyModelReady();
  }, [asrModel]);

  const loadDevices = async () => {
    try {
      const devList = await invoke<string[]>("get_audio_devices");
      setDevices(devList);
    } catch (e) {
      console.error("Failed to load devices:", e);
    }
  };

  const checkAccuracyModelReady = async () => {
    try {
      const info = await invoke<AccuracyModelInfo>("check_accuracy_model_ready");
      setModelInfo(info);
    } catch (e) {
      console.warn("Failed to check accuracy model readiness:", e);
      setModelInfo(null);
    }
  };

  const handleAudioChange = (field: string, value: any) => {
    updateConfig({
      ...config,
      audio: { ...config.audio, [field]: value }
    });
  };

  const copyToClipboard = async (text: string): Promise<boolean> => {
    try {
      if (navigator.clipboard && navigator.clipboard.writeText) {
        await navigator.clipboard.writeText(text);
        return true;
      }
    } catch (e) {
      console.warn("Clipboard API failed:", e);
    }

    const textarea = document.createElement("textarea");
    textarea.value = text;
    textarea.style.position = "fixed";
    textarea.style.opacity = "0";
    textarea.style.pointerEvents = "none";
    document.body.appendChild(textarea);
    textarea.select();
    try {
      const ok = document.execCommand("copy");
      return ok;
    } catch (e) {
      console.warn("execCommand copy failed:", e);
      return false;
    } finally {
      document.body.removeChild(textarea);
    }
  };

  const handleCopyUrl = async () => {
    const url = modelInfo?.download_url;
    if (!url) return;
    const ok = await copyToClipboard(url);
    if (ok) {
      setCopiedField('url');
      setTimeout(() => setCopiedField(null), 2000);
    }
  };

  const handleCopyPath = async () => {
    const path = modelInfo?.model_dir;
    if (!path) return;
    const ok = await copyToClipboard(path);
    if (ok) {
      setCopiedField('dir');
      setTimeout(() => setCopiedField(null), 2000);
    } else {
      window.alert(t.voice_asr_model_manual_download);
    }
  };

  const showAccuracyAlert = asrModel === "accuracy" && modelInfo && !modelInfo.ready;

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
        <h3 className="section-title">{t.voice_input_language}</h3>
        <div className="card">
          <p className="form-hint" style={{ marginBottom: '12px' }}>{t.voice_input_language_hint}</p>
          <select
            value={config.audio.transcription_language || 'zh'}
            onChange={(e) => handleAudioChange('transcription_language', e.target.value)}
            className="select-input"
          >
            <option value="zh">{t.voice_language_zh}</option>
            <option value="en">{t.voice_language_en}</option>
            <option value="ja">{t.voice_language_ja}</option>
            <option value="ko">{t.voice_language_ko}</option>
            <option value="yue">{t.voice_language_yue}</option>
          </select>
        </div>
      </section>

      <section className="settings-section">
        <h3 className="section-title">{t.voice_asr_model}</h3>
        <div className="card">
          <div className="radio-group">
            <label className={`radio-card ${asrModel === 'performance' ? 'active' : ''}`}>
              <input
                type="radio"
                name="asr_model"
                checked={asrModel === 'performance'}
                onChange={() => handleAudioChange('asr_model', 'performance')}
                className="radio-input"
              />
              <span className="custom-radio"></span>
              <div className="radio-content">
                <span className="radio-title">{t.voice_asr_model_performance}</span>
              </div>
            </label>
            <label className={`radio-card ${asrModel === 'accuracy' ? 'active' : ''}`}>
              <input
                type="radio"
                name="asr_model"
                checked={asrModel === 'accuracy'}
                onChange={() => handleAudioChange('asr_model', 'accuracy')}
                className="radio-input"
              />
              <span className="custom-radio"></span>
              <div className="radio-content">
                <span className="radio-title">{t.voice_asr_model_accuracy}</span>
              </div>
            </label>
          </div>

          {showAccuracyAlert && (
            <div className="asr-model-alert">
              <p className="asr-model-alert-title">{t.voice_asr_model_download_required}</p>
              <div className="asr-model-field">
                <span className="asr-model-label">{t.voice_asr_model_download_url}</span>
                <button
                  type="button"
                  onClick={() => invoke('open_url_in_browser', { url: modelInfo!.download_url }).catch(() => {})}
                  className="btn btn-primary btn-sm"
                >
                  {t.voice_asr_model_open_download}
                </button>
              </div>
              <div className="asr-model-field">
                <span className="asr-model-label">{t.voice_asr_model_download_url}</span>
                <div className="asr-model-path-row">
                  <code className="asr-model-path">{modelInfo!.download_url}</code>
                  <button
                    type="button"
                    onClick={handleCopyUrl}
                    className="btn btn-secondary btn-sm"
                  >
                    {copiedField === 'url' ? t.voice_copied : t.voice_copy}
                  </button>
                </div>
              </div>
              <div className="asr-model-field">
                <span className="asr-model-label">{t.voice_asr_model_target_dir}</span>
                <div className="asr-model-path-row">
                  <code className="asr-model-path">{modelInfo!.model_dir}</code>
                  <button
                    type="button"
                    onClick={handleCopyPath}
                    className="btn btn-secondary btn-sm"
                  >
                    {copiedField === 'dir' ? t.voice_copied : t.voice_copy}
                  </button>
                </div>
              </div>
              <p className="form-hint" style={{ marginTop: '8px' }}>
                {t.voice_asr_model_manual_download}
              </p>
            </div>
          )}
        </div>
      </section>

      <section className="settings-section">
        <h3 className="section-title">{t.voice_recognition_output}</h3>
        <div className="card">
          <label className="toggle-switch" style={{ border: 'none', boxShadow: 'none', padding: '8px 0', marginBottom: '12px' }}>
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
          <div className="form-group" style={{ marginTop: '16px' }}>
            <span className="form-label">{t.voice_chinese_output}</span>
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
