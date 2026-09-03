import React, { useState, useRef, useLayoutEffect } from 'react';
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
  'ShiftRight': 0xA1, 'ControlLeft': 0xA2, 'AltLeft': 0xA4, 'ShiftLeft': 0xA0,
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
  0xA1: 'Right Shift', 0xA2: 'Left Ctrl', 0xA4: 'Left Alt', 0xA0: 'Left Shift',
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



export function voiceKeySet(vkCode: number, modifiers: number): Set<number> {
  const set = new Set<number>();
  set.add(vkCode);
  if (modifiers & 0x0001) { set.add(0xA4); set.add(0xA5); }
  if (modifiers & 0x0002) { set.add(0xA2); set.add(0xA3); }
  if (modifiers & 0x0004) { set.add(0xA0); set.add(0xA1); }
  if (modifiers & 0x0008) { set.add(0x5B); set.add(0x5C); }
  return set;
}

export function translationKeySet(vkCode: number): Set<number> {
  const set = new Set<number>();
  if (vkCode !== 0) set.add(vkCode);
  return set;
}

export function keysOverlap(a: Set<number>, b: Set<number>): boolean {
  if (a.size === 0 || b.size === 0) return false;
  for (const v of a) if (b.has(v)) return true;
  return false;
}

const MODIFIER_CODES = new Set([
  'ControlLeft', 'ControlRight',
  'AltLeft', 'AltRight',
  'ShiftLeft', 'ShiftRight',
  'MetaLeft', 'MetaRight',
]);

const MODIFIER_DISPLAY_ORDER = [
  'ControlLeft', 'ControlRight',
  'AltLeft', 'AltRight',
  'ShiftLeft', 'ShiftRight',
  'MetaLeft', 'MetaRight',
];

function pressedModifierLabels(pressed: Set<string>): string {
  const labels: string[] = [];
  for (const c of MODIFIER_DISPLAY_ORDER) {
    if (pressed.has(c)) {
      const vk = CODE_TO_VK[c];
      if (vk !== undefined && VK_TO_LABEL[vk]) labels.push(VK_TO_LABEL[vk]);
    }
  }
  return labels.join('+');
}

const HotkeySettingsPage: React.FC<Props> = ({ config, updateConfig }) => {
  const t = getTranslations(config.ui_language);
  const [activeSubTab, setActiveSubTab] = useState<'voice' | 'translation'>('voice');

  const [isRecordingVoice, setIsRecordingVoice] = useState(false);
  const [pendingHotkey, setPendingHotkey] = useState<{vk: number, mod: number} | null>(null);
  const [dupConflict, setDupConflict] = useState<{ voice: string, translation: string } | null>(null);
  const voiceInputRef = useRef<HTMLDivElement>(null);
  const pressedModsRef = useRef<Set<string>>(new Set());
  const hadNonModifierKeyRef = useRef(false);
  const altGrSynthCtrlActiveRef = useRef(false);
  const voiceFinalizedRef = useRef(false);
  const [voiceRecordingPreview, setVoiceRecordingPreview] = useState<string>('');

  const [isRecordingTranslation, setIsRecordingTranslation] = useState(false);
  const translationInputRef = useRef<HTMLDivElement>(null);
  const translationFinalizedRef = useRef(false);
  // HOTKEY-079: translation-side exclusive. AltGr always synthesizes
  // ControlLeft BEFORE AltRight, and at the ControlLeft keyDown itself
  // getModifierState('AltGraph') is still false — there is not enough
  // information to tell it apart from a real Left Ctrl press at that instant.
  // So the verdict is deferred: pending=true means "ControlLeft seen, waiting
  // for either an AltRight takeover (record Right Alt) or its own keyUp
  // (record Left Ctrl)". Translation-side exclusive per HOTKEY-060 (sides
  // never share state); sole clear point: resetTranslationRecordingState().
  const translationPendingCtrlRef = useRef(false);

  const translation = config.translation ?? { enabled: false, vk_code: 0, display_name: '', target_language: 'Chinese' };

  const resetVoiceRecordingState = () => {
    pressedModsRef.current.clear();
    hadNonModifierKeyRef.current = false;
    altGrSynthCtrlActiveRef.current = false;
    voiceFinalizedRef.current = false;
    setVoiceRecordingPreview('');
  };

  const resetTranslationRecordingState = () => {
    translationFinalizedRef.current = false;
    // HOTKEY-079: pending must die with the session (Escape / blur / finalize
    // all route through here). Leaking it would misroute the next session's
    // first ControlLeft — same lifecycle trap HOTKEY-078 fixed on voice side.
    translationPendingCtrlRef.current = false;
  };

  // HOTKEY-060: shared conflict-detection + finalize + reset flow.
  // The caller supplies its own per-session finalized ref and reset callback so
  // voice and translation recordings never cross-interfere.
  type HotkeySide = 'voice' | 'translation';
  const applyHotkeyIfNoDupConflict = (
    finalizedRef: React.MutableRefObject<boolean>,
    side: HotkeySide,
    setRecording: (v: boolean) => void,
    resetRecording: () => void,
    voiceVkCode: number,
    voiceModifiers: number,
    translationVkCode: number,
  ) => {
    if (finalizedRef.current) return;
    finalizedRef.current = true;

    const vSet = voiceKeySet(voiceVkCode, voiceModifiers);
    const tSet = translationKeySet(translationVkCode);
    if (keysOverlap(vSet, tSet)) {
      setRecording(false);
      setDupConflict({
        voice: getHotkeyDisplayName(voiceVkCode, voiceModifiers),
        translation: translation.display_name || VK_TO_LABEL[translationVkCode] || translationVkCode.toString(),
      });
      resetRecording();
      return;
    }

    if (side === 'voice') {
      const newHotkey = {
        ...config.hotkey,
        vk_code: voiceVkCode,
        modifiers: voiceModifiers,
        display_name: getHotkeyDisplayName(voiceVkCode, voiceModifiers),
      };
      updateConfig({ ...config, hotkey: newHotkey });
    } else {
      const displayName = VK_TO_LABEL[translationVkCode] || translationVkCode.toString();
      updateConfig({
        ...config,
        translation: { ...translation, vk_code: translationVkCode, display_name: displayName }
      });
    }
    setRecording(false);
    resetRecording();
  };

  const checkAndApplyVoiceHotkey = async (vkCode: number, modifiers: number) => {
    try {
      const available = await invoke<boolean>('check_hotkey_available', {
        vk_code: vkCode,
        modifiers: modifiers,
      });
      if (available) {
        applyHotkeyIfNoDupConflict(
          voiceFinalizedRef,
          'voice',
          setIsRecordingVoice,
          resetVoiceRecordingState,
          vkCode,
          modifiers,
          translation.vk_code,
        );
      } else {
        setIsRecordingVoice(false);
        setPendingHotkey({ vk: vkCode, mod: modifiers });
        resetVoiceRecordingState();
      }
    } catch {
      applyHotkeyIfNoDupConflict(
        voiceFinalizedRef,
        'voice',
        setIsRecordingVoice,
        resetVoiceRecordingState,
        vkCode,
        modifiers,
        translation.vk_code,
      );
    }
  };

  const handleVoiceHotkeyKeyDown = (e: React.KeyboardEvent) => {
    e.preventDefault();
    e.stopPropagation();
    if (voiceFinalizedRef.current) return;
    const code = e.code;

    if (code === 'Escape') {
      setIsRecordingVoice(false);
      resetVoiceRecordingState();
      return;
    }

    if (MODIFIER_CODES.has(code)) {
      if (code === 'AltRight') {
        pressedModsRef.current.delete('ControlLeft');
        altGrSynthCtrlActiveRef.current = true;
      }
      if (!pressedModsRef.current.has(code)) {
        pressedModsRef.current.add(code);
        setVoiceRecordingPreview(pressedModifierLabels(pressedModsRef.current));
      }
      return;
    }

    const vkCode = CODE_TO_VK[code];
    if (vkCode === undefined) {
      console.warn('Unknown key code:', code);
      return;
    }

    const isAltGr = e.getModifierState('AltGraph');
    let modifiers = 0;
    if (e.altKey) modifiers |= 0x0001;
    if (e.ctrlKey && !isAltGr) modifiers |= 0x0002;
    if (e.shiftKey) modifiers |= 0x0004;
    if (e.getModifierState('Meta')) modifiers |= 0x0008;

    hadNonModifierKeyRef.current = true;
    checkAndApplyVoiceHotkey(vkCode, modifiers);
  };

  const handleVoiceHotkeyKeyUp = (e: React.KeyboardEvent) => {
    e.preventDefault();
    e.stopPropagation();
    if (voiceFinalizedRef.current) return;
    if (!isRecordingVoice) return;
    const code = e.code;

    // HOTKEY-078: altGrSynthCtrlActiveRef must stay true until the trailing
    // synthetic ControlLeft keyUp arrives (it is always dispatched AFTER
    // AltRight keyUp). Clearing it on AltRight keyUp re-opens the :279
    // suppression too early and lets the synthetic Ctrl be recorded as a
    // second hotkey. Sole clear point: resetVoiceRecordingState().
    if (code === 'ControlLeft' && altGrSynthCtrlActiveRef.current) {
      pressedModsRef.current.delete(code);
      return;
    }

    if (!MODIFIER_CODES.has(code)) return;

    const altGrSynthWasActive = altGrSynthCtrlActiveRef.current;

    if (hadNonModifierKeyRef.current) {
      pressedModsRef.current.delete(code);
      return;
    }

    pressedModsRef.current.delete(code);
    if (pressedModsRef.current.size > 0) {
      setVoiceRecordingPreview(pressedModifierLabels(pressedModsRef.current));
      return;
    }

    const singleVk = CODE_TO_VK[code];
    if (singleVk === undefined) return;

    const isAltGrUp = e.getModifierState('AltGraph');
    let modifiers = 0;
    if (e.altKey) modifiers |= 0x0001;
    if (e.ctrlKey && !isAltGrUp && !altGrSynthWasActive) modifiers |= 0x0002;
    if (e.shiftKey) modifiers |= 0x0004;
    if (e.getModifierState('Meta')) modifiers |= 0x0008;

    checkAndApplyVoiceHotkey(singleVk, modifiers);
  };

  useLayoutEffect(() => {
    if (isRecordingVoice) {
      resetVoiceRecordingState();
      voiceInputRef.current?.focus();
    }
  }, [isRecordingVoice]);

  const startRecordingVoice = () => {
    setIsRecordingVoice(true);
  };

  const handleVoiceModeChange = (mode: string) => {
    updateConfig({
      ...config,
      hotkey: { ...config.hotkey, mode }
    });
  };

  const handleTranslationHotkeyKeyDown = (e: React.KeyboardEvent) => {
    e.preventDefault();
    e.stopPropagation();
    const code = e.code;

    if (code === 'Escape') {
      setIsRecordingTranslation(false);
      resetTranslationRecordingState();
      return;
    }

    const vkCode = CODE_TO_VK[code];
    if (vkCode === undefined) {
      console.warn('Unknown key code:', code);
      return;
    }

    // HOTKEY-079: AltGr synthesizes ControlLeft then AltRight; at the
    // ControlLeft keyDown itself AltGraph is still false, so the verdict is
    // deferred here and resolved by AltRight (record Right Alt) or the
    // ControlLeft keyUp (record Left Ctrl).
    if (code === 'ControlLeft') {
      translationPendingCtrlRef.current = true;
      return;
    }
    if (code === 'AltRight' && translationPendingCtrlRef.current) {
      translationPendingCtrlRef.current = false;
      applyHotkeyIfNoDupConflict(
        translationFinalizedRef,
        'translation',
        setIsRecordingTranslation,
        resetTranslationRecordingState,
        config.hotkey.vk_code,
        config.hotkey.modifiers,
        0xA5,
      );
      return;
    }

    applyHotkeyIfNoDupConflict(
      translationFinalizedRef,
      'translation',
      setIsRecordingTranslation,
      resetTranslationRecordingState,
      config.hotkey.vk_code,
      config.hotkey.modifiers,
      vkCode,
    );
  };

  const handleTranslationHotkeyKeyUp = (e: React.KeyboardEvent) => {
    e.preventDefault();
    e.stopPropagation();
    if (translationFinalizedRef.current) return;
    if (!isRecordingTranslation) return;
    const code = e.code;

    // HOTKEY-079: pending still true at ControlLeft keyUp = no AltRight ever
    // arrived = a real standalone Left Ctrl press. Record it now. Any other
    // keyUp is ignored (translation is single-key semantics).
    if (code === 'ControlLeft' && translationPendingCtrlRef.current) {
      translationPendingCtrlRef.current = false;
      applyHotkeyIfNoDupConflict(
        translationFinalizedRef,
        'translation',
        setIsRecordingTranslation,
        resetTranslationRecordingState,
        config.hotkey.vk_code,
        config.hotkey.modifiers,
        0xA2,
      );
    }
  };

  const startRecordingTranslation = () => {
    setIsRecordingTranslation(true);
  };

  useLayoutEffect(() => {
    if (isRecordingTranslation) {
      resetTranslationRecordingState();
      translationInputRef.current?.focus();
    }
  }, [isRecordingTranslation]);

  const handleTranslationEnabledChange = (enabled: boolean) => {
    updateConfig({
      ...config,
      translation: { ...translation, enabled }
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
                <div
                  className="hotkey-key-btn hotkey-key-listening"
                  tabIndex={0}
                  ref={voiceInputRef}
                  autoFocus
                  onKeyDown={handleVoiceHotkeyKeyDown}
                  onKeyUp={handleVoiceHotkeyKeyUp}
                  onBlur={() => { setIsRecordingVoice(false); resetVoiceRecordingState(); }}
                >{voiceRecordingPreview || t.hotkey_press_new}</div>
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
                <div
                  className="hotkey-key-btn hotkey-key-listening"
                  tabIndex={0}
                  ref={translationInputRef}
                  autoFocus
                  onKeyDown={handleTranslationHotkeyKeyDown}
                  onKeyUp={handleTranslationHotkeyKeyUp}
                  onBlur={() => { setIsRecordingTranslation(false); resetTranslationRecordingState(); }}
                >{t.hotkey_press_translation}</div>
              )}
              <span className="hotkey-key-hint">{t.hotkey_set_translation}</span>
            </div>
            {translation.enabled && translation.vk_code === 0 && (
              <div style={{ color: 'var(--status-warning)', fontSize: '13px', marginTop: '8px' }}>{t.hotkey_set_first}</div>
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
              <button className="btn btn-primary" onClick={() => { applyHotkeyIfNoDupConflict(voiceFinalizedRef, 'voice', setIsRecordingVoice, resetVoiceRecordingState, pendingHotkey.vk, pendingHotkey.mod, translation.vk_code); setPendingHotkey(null); }}>{t.hotkey_use_anyway}</button>
            </div>
          </div>
        </div>
      )}

      {dupConflict && (
        <div className="modal-overlay" onClick={() => setDupConflict(null)}>
          <div className="modal-dialog" role="dialog" onClick={e => e.stopPropagation()}>
            <div className="modal-header">
              <span className="modal-title">{t.hotkey_dup_title}</span>
              <button className="modal-close" onClick={() => setDupConflict(null)}>×</button>
            </div>
            <div className="modal-body">
              <p style={{ margin: 0, lineHeight: '1.6' }}>
                {t.hotkey_dup_prefix}{dupConflict.voice}{t.hotkey_dup_infix}{dupConflict.translation}{t.hotkey_dup_suffix}
              </p>
            </div>
            <div className="modal-footer">
              <button className="btn btn-primary" onClick={() => setDupConflict(null)}>{t.hotkey_dup_ack}</button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};

export default HotkeySettingsPage;
