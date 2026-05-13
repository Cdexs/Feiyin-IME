import React, { useState, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { getTranslations } from '../i18n';

interface Props {
  config: any;
  updateConfig: (cfg: any) => void;
}

const CODE_TO_VK: { [key: string]: number } = {
  'KeyA': 0x41, 'KeyB': 0x42, 'KeyC': 0x43, 'KeyD': 0x44, 'KeyE': 0x45,
  'KeyF': 0x46, 'KeyG': 0x47, 'KeyH': 0x48, 'KeyI': 0x49, 'KeyJ': 0x4A,
  'KeyK': 0x4B, 'KeyL': 0x4C, 'KeyM': 0x4D, 'KeyN': 0x4E, 'KeyO': 0x4F,
  'KeyP': 0x50, 'KeyQ': 0x51, 'KeyR': 0x52, 'KeyS': 0x53, 'KeyT': 0x54,
  'KeyU': 0x55, 'KeyV': 0x56, 'KeyW': 0x57, 'KeyX': 0x58, 'KeyY': 0x59,
  'KeyZ': 0x5A,
  'Digit0': 0x30, 'Digit1': 0x31, 'Digit2': 0x32, 'Digit3': 0x33, 'Digit4': 0x34,
  'Digit5': 0x35, 'Digit6': 0x36, 'Digit7': 0x37, 'Digit8': 0x38, 'Digit9': 0x39,
  'F1': 0x70, 'F2': 0x71, 'F3': 0x72, 'F4': 0x73, 'F5': 0x74,
  'F6': 0x75, 'F7': 0x76, 'F8': 0x77, 'F9': 0x78, 'F10': 0x79,
  'F11': 0x7A, 'F12': 0x7B,
  'ControlRight': 0xA3, 'AltRight': 0xA5,
  'ShiftRight': 0xA0, 'ControlLeft': 0xA2, 'AltLeft': 0xA4, 'ShiftLeft': 0xA1,
  'Space': 0x20, 'Enter': 0x0D, 'Backspace': 0x08, 'Tab': 0x09,
  'Escape': 0x1B, 'Insert': 0x2D, 'Delete': 0x2E, 'Home': 0x24, 'End': 0x23,
  'PageUp': 0x21, 'PageDown': 0x22,
  'ArrowUp': 0x26, 'ArrowDown': 0x28, 'ArrowLeft': 0x25, 'ArrowRight': 0x27,
};

const VK_TO_LABEL: { [key: number]: string } = {
  0x70: 'F1', 0x71: 'F2', 0x72: 'F3', 0x73: 'F4',
  0x74: 'F5', 0x75: 'F6', 0x76: 'F7', 0x77: 'F8',
  0x78: 'F9', 0x79: 'F10', 0x7A: 'F11', 0x7B: 'F12',
  0x41: 'A', 0x42: 'B', 0x43: 'C', 0x44: 'D', 0x45: 'E',
  0x46: 'F', 0x47: 'G', 0x48: 'H', 0x49: 'I', 0x4A: 'J',
  0x4B: 'K', 0x4C: 'L', 0x4D: 'M', 0x4E: 'N', 0x4F: 'O',
  0x50: 'P', 0x51: 'Q', 0x52: 'R', 0x53: 'S', 0x54: 'T',
  0x55: 'U', 0x56: 'V', 0x57: 'W', 0x58: 'X', 0x59: 'Y',
  0x5A: 'Z',
  0x30: '0', 0x31: '1', 0x32: '2', 0x33: '3', 0x34: '4',
  0x35: '5', 0x36: '6', 0x37: '7', 0x38: '8', 0x39: '9',
  0xA3: 'Right Ctrl', 0xA5: 'Right Alt',
  0xA0: 'Right Shift', 0xA2: 'Left Ctrl', 0xA4: 'Left Alt', 0xA1: 'Left Shift',
};

const MOD_LABELS: { [key: number]: string } = {
  0x0001: 'Alt',
  0x0002: 'Ctrl',
  0x0004: 'Shift',
  0x0008: 'Win',
};

function getHotkeyDisplayName(vkCode: number, modifiers: number): string {
  const parts: string[] = [];
  if (modifiers & 0x0002) parts.push(MOD_LABELS[0x0002]);
  if (modifiers & 0x0001) parts.push(MOD_LABELS[0x0001]);
  if (modifiers & 0x0004) parts.push(MOD_LABELS[0x0004]);
  if (modifiers & 0x0008) parts.push(MOD_LABELS[0x0008]);

  const keyName = VK_TO_LABEL[vkCode];
  if (keyName) {
    parts.push(keyName);
  } else {
    parts.push(vkCode.toString());
  }
  return parts.join('+');
}

const TRANSLATION_SINGLE_KEYS = [0xA3, 0xA2, 0xA5, 0xA4];

const HotkeySettingsPage: React.FC<Props> = ({ config, updateConfig }) => {
  const t = getTranslations(config.ui_language);
  const [activeSubTab, setActiveSubTab] = useState<'voice' | 'translation'>('voice');

  const [isRecordingVoice, setIsRecordingVoice] = useState(false);
  const [pendingHotkey, setPendingHotkey] = useState<{vk: number, mod: number} | null>(null);
  const voiceInputRef = useRef<HTMLDivElement>(null);

  const [isRecordingTranslation, setIsRecordingTranslation] = useState(false);
  const translationInputRef = useRef<HTMLDivElement>(null);

  const translation = config.translation ?? { enabled: false, vk_code: 0, display_name: '', target_language: 'Chinese' };
  const transcriptionLang = String(config.audio?.transcription_language ?? 'zh').toLowerCase();
  const sourceIsZh = transcriptionLang.startsWith('zh');
  const sourceIsEn = transcriptionLang === 'en' || transcriptionLang.startsWith('en-');
  const targetLanguageOptions = React.useMemo(() => {
    if (sourceIsZh) return ['English'];
    if (sourceIsEn) return ['Chinese'];
    return ['Chinese', 'English'];
  }, [sourceIsZh, sourceIsEn]);
  const effectiveTargetLanguage = targetLanguageOptions.includes(translation.target_language)
    ? translation.target_language
    : targetLanguageOptions[0];
  const targetLanguageHint = sourceIsZh
    ? t.hotkey_translation_hint_zh
    : sourceIsEn
      ? t.hotkey_translation_hint_en
      : null;

  React.useEffect(() => {
    if (translation.target_language !== effectiveTargetLanguage) {
      updateConfig({
        ...config,
        translation: { ...translation, target_language: effectiveTargetLanguage }
      });
    }
  }, [translation.target_language, effectiveTargetLanguage]);

  const applyVoiceHotkey = (vkCode: number, modifiers: number) => {
    const newHotkey = {
      ...config.hotkey,
      vk_code: vkCode,
      modifiers: modifiers,
      display_name: getHotkeyDisplayName(vkCode, modifiers),
    };
    updateConfig({ ...config, hotkey: newHotkey });
    setIsRecordingVoice(false);
  };

  const checkAndApplyVoiceHotkey = async (vkCode: number, modifiers: number) => {
    try {
      const available = await invoke<boolean>('check_hotkey_available', {
        vk_code: vkCode,
        modifiers: modifiers,
      });
      if (available) {
        applyVoiceHotkey(vkCode, modifiers);
      } else {
        setIsRecordingVoice(false);
        setPendingHotkey({ vk: vkCode, mod: modifiers });
      }
    } catch {
      applyVoiceHotkey(vkCode, modifiers);
    }
  };

  const handleVoiceHotkeyKeyDown = (e: React.KeyboardEvent) => {
    e.preventDefault();
    e.stopPropagation();
    const code = e.code;

    if (code === 'Escape') {
      setIsRecordingVoice(false);
      return;
    }

    if (code === 'ControlRight') {
      applyVoiceHotkey(0xA3, 0);
      return;
    }
    if (code === 'AltRight') {
      applyVoiceHotkey(0xA5, 0);
      return;
    }

    if (['ControlLeft', 'AltLeft', 'ShiftLeft', 'ShiftRight', 'MetaLeft', 'MetaRight'].includes(code)) {
      return;
    }

    const vkCode = CODE_TO_VK[code];
    if (vkCode === undefined) {
      console.warn('Unknown key code:', code);
      return;
    }

    let modifiers = 0;
    if (e.altKey) modifiers |= 0x0001;
    if (e.ctrlKey) modifiers |= 0x0002;
    if (e.shiftKey) modifiers |= 0x0004;

    checkAndApplyVoiceHotkey(vkCode, modifiers);
  };

  const startRecordingVoice = () => {
    setIsRecordingVoice(true);
    setTimeout(() => voiceInputRef.current?.focus(), 50);
  };

  const handleVoiceModeChange = (mode: string) => {
    updateConfig({
      ...config,
      hotkey: { ...config.hotkey, mode }
    });
  };

  const applyTranslationHotkey = (vkCode: number) => {
    const displayName = VK_TO_LABEL[vkCode] || vkCode.toString();
    updateConfig({
      ...config,
      translation: { ...translation, vk_code: vkCode, display_name: displayName }
    });
    setIsRecordingTranslation(false);
  };

  const handleTranslationHotkeyKeyDown = (e: React.KeyboardEvent) => {
    e.preventDefault();
    e.stopPropagation();
    const code = e.code;

    if (code === 'Escape') {
      setIsRecordingTranslation(false);
      return;
    }

    const vkCode = CODE_TO_VK[code];
    if (vkCode === undefined) {
      console.warn('Unknown key code:', code);
      return;
    }

    if (!TRANSLATION_SINGLE_KEYS.includes(vkCode)) {
      applyTranslationHotkey(vkCode);
      return;
    }

    applyTranslationHotkey(vkCode);
  };

  const startRecordingTranslation = () => {
    setIsRecordingTranslation(true);
    setTimeout(() => translationInputRef.current?.focus(), 50);
  };

  const handleTranslationEnabledChange = (enabled: boolean) => {
    updateConfig({
      ...config,
      translation: { ...translation, enabled }
    });
  };

  const handleTargetLanguageChange = (target_language: string) => {
    updateConfig({
      ...config,
      translation: { ...translation, target_language }
    });
  };

  const currentVoiceDisplayName = getHotkeyDisplayName(
    config.hotkey.vk_code,
    config.hotkey.modifiers
  );

  const currentTranslationDisplayName = translation.display_name || t.hotkey_not_set;

  return (
    <div className="settings-page">
      <h2 className="page-title">{t.hotkey_title}</h2>

      <div className="sub-tab-bar">
        <button
          className={`sub-tab-btn ${activeSubTab === 'voice' ? 'active' : ''}`}
          onClick={() => setActiveSubTab('voice')}
        >{t.hotkey_voice_tab}</button>
        <button
          className={`sub-tab-btn ${activeSubTab === 'translation' ? 'active' : ''}`}
          onClick={() => setActiveSubTab('translation')}
        >{t.hotkey_translation_tab}</button>
      </div>

      {activeSubTab === 'voice' && (
        <div className="sub-tab-content">
          <div className="card" style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', padding: '24px 16px' }}>
            <div className="hotkey-key-wrapper">
              {!isRecordingVoice ? (
                <button className="hotkey-key-btn" onClick={startRecordingVoice}>{currentVoiceDisplayName}</button>
              ) : (
                <div className="hotkey-key-btn hotkey-key-listening" tabIndex={0} ref={voiceInputRef} onKeyDown={handleVoiceHotkeyKeyDown}>{t.hotkey_press_new}</div>
              )}
              <span className="hotkey-key-hint">{t.hotkey_click_to_change}</span>
            </div>
          </div>

          <div className="card" style={{ marginTop: '12px' }}>
            <div className="section-subtitle" style={{ marginBottom: '12px' }}>{t.hotkey_trigger_mode}</div>
            <div className="radio-group">
              <label className={`radio-card ${config.hotkey.mode === 'Toggle' ? 'active' : ''}`}>
                <input type="radio" name="hotkey_mode" checked={config.hotkey.mode === 'Toggle'} onChange={() => handleVoiceModeChange('Toggle')} className="radio-input" />
                <span className="custom-radio"></span>
                <div className="radio-content">
                  <span className="radio-title">{t.hotkey_toggle}</span>
                  <span className="radio-desc">{t.hotkey_toggle_desc}</span>
                </div>
              </label>
              <label className={`radio-card ${config.hotkey.mode === 'PushToTalk' ? 'active' : ''}`}>
                <input type="radio" name="hotkey_mode" checked={config.hotkey.mode === 'PushToTalk'} onChange={() => handleVoiceModeChange('PushToTalk')} className="radio-input" />
                <span className="custom-radio"></span>
                <div className="radio-content">
                  <span className="radio-title">{t.hotkey_ptt}</span>
                  <span className="radio-desc">{t.hotkey_ptt_desc}</span>
                </div>
              </label>
            </div>
          </div>
        </div>
      )}

      {activeSubTab === 'translation' && (
        <div className="sub-tab-content">
          <div className="card" style={{ marginBottom: '12px' }}>
            <label className="toggle-switch card" style={{ border: 'none', boxShadow: 'none', padding: '8px 0', marginBottom: 0 }}>
              <input type="checkbox" checked={translation.enabled} onChange={(e) => handleTranslationEnabledChange(e.target.checked)} className="toggle-input" />
              <span className="toggle-track"></span>
              <span className="toggle-label">{t.hotkey_enable_translation}</span>
            </label>
          </div>

          <div className="card" style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', padding: '24px 16px', marginBottom: '12px' }}>
            <div className="hotkey-key-wrapper">
              {!isRecordingTranslation ? (
                <button className="hotkey-key-btn" onClick={startRecordingTranslation}>{currentTranslationDisplayName}</button>
              ) : (
                <div className="hotkey-key-btn hotkey-key-listening" tabIndex={0} ref={translationInputRef} onKeyDown={handleTranslationHotkeyKeyDown}>{t.hotkey_press_translation}</div>
              )}
              <span className="hotkey-key-hint">{t.hotkey_set_translation}</span>
            </div>
            {translation.enabled && translation.vk_code === 0 && (
              <div style={{ color: 'var(--status-warning)', fontSize: '13px', marginTop: '8px' }}>{t.hotkey_set_first}</div>
            )}
          </div>

          <div className="card" style={{ marginBottom: '12px' }}>
            <div className="section-subtitle" style={{ marginBottom: '12px' }}>{t.hotkey_target_language}</div>
            <select value={effectiveTargetLanguage} onChange={(e) => handleTargetLanguageChange(e.target.value)} className="select-input">
              {targetLanguageOptions.includes('Chinese') && <option value="Chinese">{t.hotkey_target_chinese}</option>}
              {targetLanguageOptions.includes('English') && <option value="English">{t.hotkey_target_english}</option>}
            </select>
            {targetLanguageHint && (
              <p style={{ color: 'rgba(0, 0, 0, 0.5)', fontSize: '12px', lineHeight: '1.5', margin: '8px 0 0' }}>{targetLanguageHint}</p>
            )}
          </div>

          <div className="card" style={{ padding: '16px' }}>
            <p style={{ color: 'rgba(0, 0, 0, 0.5)', fontSize: '13px', lineHeight: '1.6', margin: 0 }}>{t.hotkey_translation_usage}</p>
          </div>
        </div>
      )}

      {pendingHotkey && (
        <div className="modal-overlay" onClick={() => setPendingHotkey(null)}>
          <div className="modal-dialog" role="dialog" onClick={e => e.stopPropagation()}>
            <div className="modal-header">
              <span className="modal-title">{t.hotkey_conflict}</span>
              <button className="modal-close" onClick={() => setPendingHotkey(null)}>×</button>
            </div>
            <div className="modal-body">
              <p style={{ margin: 0, lineHeight: '1.6' }}>
                {t.hotkey_conflict_prefix}{getHotkeyDisplayName(pendingHotkey.vk, pendingHotkey.mod)}{t.hotkey_conflict_suffix}
              </p>
            </div>
            <div className="modal-footer">
              <button className="btn btn-secondary" onClick={() => setPendingHotkey(null)}>{t.hotkey_cancel}</button>
              <button className="btn btn-primary" onClick={() => { applyVoiceHotkey(pendingHotkey.vk, pendingHotkey.mod); setPendingHotkey(null); }}>{t.hotkey_use_anyway}</button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};

export default HotkeySettingsPage;
