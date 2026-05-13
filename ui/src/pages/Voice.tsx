import React, { useState, useEffect } from 'react';
import { invoke } from "@tauri-apps/api/core";
import { getTranslations } from '../i18n';

interface Props {
  config: any;
  updateConfig: (cfg: any) => void;
}

const VoicePage: React.FC<Props> = ({ config, updateConfig }) => {
  const [devices, setDevices] = useState<string[]>([]);
  const t = getTranslations(config.ui_language);

  useEffect(() => {
    loadDevices();
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
            {devices.map(d => <option key={d} value={d}>{d}</option>)}
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
