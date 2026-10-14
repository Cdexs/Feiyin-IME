// SC-LLM-485 / SC-PUNCT-486（Gavin 10-14）：功能开关后面的「（快捷方式: 右Ctrl+1）」+ 设置对话框。
//
// 规则与主程序逐条对应（改一边必须改另一边）：
//   - 合法组合：左右 Ctrl / Alt 中**恰好一个** + 字母或数字   ↔ src/config/mod.rs `ShortcutBinding::is_valid`
//   - 与热键冲突                                            ↔ `ShortcutBinding::conflict_with`
//   - 两个快捷方式相同时只有「识别输出优化」生效             ↔ `AppConfig::active_shortcuts`
//   - 默认 右Ctrl+1 / 右Ctrl+2                              ↔ `default_llm_toggle_shortcut` / `default_strip_trailing_shortcut`
import React, { useEffect, useLayoutEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { getTranslations } from '../i18n';

export interface ShortcutBinding {
  modifier: number;
  key: number;
}

export type ShortcutAction = 'llm_toggle' | 'strip_trailing_toggle';

export const DEFAULT_SHORTCUTS: Record<ShortcutAction, ShortcutBinding> = {
  llm_toggle: { modifier: 0xa3, key: 0x31 },
  strip_trailing_toggle: { modifier: 0xa3, key: 0x32 },
};

/** 浏览器 KeyboardEvent.code → 分左右的功能键 VK。 */
const SIDED_MODIFIER_VK: Record<string, number> = {
  ControlLeft: 0xa2,
  ControlRight: 0xa3,
  AltLeft: 0xa4,
  AltRight: 0xa5,
};

/** 不允许夹带的其他修饰键（主程序要求「恰好」一个功能键）。 */
const OTHER_MODIFIER_CODES = new Set(['ShiftLeft', 'ShiftRight', 'MetaLeft', 'MetaRight']);

/** KeyA..KeyZ / Digit0..Digit9 → VK；其他键返回 undefined。 */
export function mainKeyVk(code: string): number | undefined {
  let m = /^Key([A-Z])$/.exec(code);
  if (m) return m[1].charCodeAt(0);
  m = /^Digit([0-9])$/.exec(code);
  if (m) return m[1].charCodeAt(0);
  return undefined;
}

export function isValidBinding(b: ShortcutBinding | undefined | null): b is ShortcutBinding {
  if (!b) return false;
  const modOk = b.modifier >= 0xa2 && b.modifier <= 0xa5;
  const keyOk = (b.key >= 0x30 && b.key <= 0x39) || (b.key >= 0x41 && b.key <= 0x5a);
  return modOk && keyOk;
}

export function sameBinding(a: ShortcutBinding | undefined, b: ShortcutBinding | undefined): boolean {
  return !!a && !!b && a.modifier === b.modifier && a.key === b.key;
}

export function bindingOf(config: any, action: ShortcutAction): ShortcutBinding {
  return config?.shortcuts?.[action] ?? DEFAULT_SHORTCUTS[action];
}

type T = ReturnType<typeof getTranslations>;

export function modifierName(vk: number, t: T): string {
  switch (vk) {
    case 0xa2: return t.shortcut_mod_left_ctrl;
    case 0xa3: return t.shortcut_mod_right_ctrl;
    case 0xa4: return t.shortcut_mod_left_alt;
    case 0xa5: return t.shortcut_mod_right_alt;
    default: return '';
  }
}

export function shortcutDisplay(b: ShortcutBinding | undefined, t: T): string {
  if (!isValidBinding(b)) return t.shortcut_not_set;
  return `${modifierName(b.modifier, t)}+${String.fromCharCode(b.key)}`;
}

export type ShortcutConflict = 'recording' | 'translation' | 'combo';

/** 与 Rust `ShortcutBinding::conflict_with` 同一判据。 */
export function shortcutConflict(b: ShortcutBinding, config: any): ShortcutConflict | null {
  if (!isValidBinding(b)) return null;
  const hotkey = config?.hotkey ?? {};
  const translation = config?.translation ?? {};
  if (hotkey.vk_code === b.modifier) return 'recording';
  if (translation.enabled && translation.vk_code && translation.vk_code === b.modifier) return 'translation';
  const modFlag = b.modifier <= 0xa3 ? 0x0002 : 0x0001;
  if (hotkey.vk_code === b.key && hotkey.modifiers === modFlag) return 'combo';
  return null;
}

const OTHER_ACTION: Record<ShortcutAction, ShortcutAction> = {
  llm_toggle: 'strip_trailing_toggle',
  strip_trailing_toggle: 'llm_toggle',
};

function actionLabel(action: ShortcutAction, t: T): string {
  return action === 'llm_toggle' ? t.llm_enable : t.voice_strip_trailing_punct;
}

/** 对话框里的红色错误：热键冲突 / 与另一个快捷方式重复。无问题返回 null。 */
export function shortcutError(b: ShortcutBinding, config: any, action: ShortcutAction, t: T): string | null {
  if (!isValidBinding(b)) return null;
  switch (shortcutConflict(b, config)) {
    case 'recording':
      return `${t.shortcut_err_recording_prefix}${modifierName(b.modifier, t)}${t.shortcut_err_recording_suffix}`;
    case 'translation':
      return `${t.shortcut_err_recording_prefix}${modifierName(b.modifier, t)}${t.shortcut_err_translation_suffix}`;
    case 'combo':
      return `${t.shortcut_err_recording_prefix}${shortcutDisplay(b, t)}${t.shortcut_err_combo_suffix}`;
    default:
      break;
  }
  const other = OTHER_ACTION[action];
  if (sameBinding(b, bindingOf(config, other))) {
    return `${t.shortcut_err_dup_prefix}${actionLabel(other, t)}${t.shortcut_err_dup_suffix}`;
  }
  return null;
}

/** 当前配置下此快捷方式是否**不生效**（与 Rust `active_shortcuts` 同一判据）。 */
export function shortcutInactive(config: any, action: ShortcutAction): boolean {
  const b = bindingOf(config, action);
  if (!isValidBinding(b)) return false; // 未设置单独显示「未设置」
  if (shortcutConflict(b, config)) return true;
  return action === 'strip_trailing_toggle' && sameBinding(b, bindingOf(config, 'llm_toggle'));
}

function setCapture(active: boolean) {
  // 录制期间让主程序放行按键（否则按下当前快捷方式会被主程序拦截、这里收不到）。
  Promise.resolve(invoke('set_shortcut_capture', { active })).catch(() => {});
}

interface Props {
  config: any;
  updateConfig: (cfg: any) => void;
  action: ShortcutAction;
}

const ShortcutSetting: React.FC<Props> = ({ config, updateConfig, action }) => {
  const t = getTranslations(config.ui_language);
  const current = bindingOf(config, action);
  const [open, setOpen] = useState(false);
  const [pending, setPending] = useState<ShortcutBinding>(current);
  const [listening, setListening] = useState(false);
  const [preview, setPreview] = useState('');
  const [captureError, setCaptureError] = useState<string | null>(null);
  const pressedRef = useRef<Set<string>>(new Set());
  const altGrSynthRef = useRef(false);
  const listeningRef = useRef(false);
  const keyRef = useRef<HTMLDivElement>(null);

  const startListening = () => {
    pressedRef.current = new Set();
    altGrSynthRef.current = false;
    setPreview('');
    setCaptureError(null);
    listeningRef.current = true;
    setListening(true);
    setCapture(true);
  };

  const stopListening = () => {
    if (!listeningRef.current) return;
    listeningRef.current = false;
    setListening(false);
    setPreview('');
    pressedRef.current = new Set();
    altGrSynthRef.current = false;
    setCapture(false);
  };

  // 卸载（切页 / 关窗）时一定释放「录制中」信号。
  useEffect(() => () => {
    if (listeningRef.current) setCapture(false);
  }, []);

  useLayoutEffect(() => {
    if (listening) keyRef.current?.focus();
  }, [listening]);

  const openDialog = () => {
    setPending(current);
    setCaptureError(null);
    setOpen(true);
  };

  const closeDialog = () => {
    stopListening();
    setOpen(false);
  };

  const modifierPreview = () =>
    Object.keys(SIDED_MODIFIER_VK)
      .filter((c) => pressedRef.current.has(c))
      .map((c) => modifierName(SIDED_MODIFIER_VK[c], t))
      .join('+');

  const handleKeyDown = (e: React.KeyboardEvent) => {
    e.preventDefault();
    e.stopPropagation();
    if (!listeningRef.current) return;
    const code = e.code;
    if (code === 'Escape') {
      stopListening();
      return;
    }
    if (code in SIDED_MODIFIER_VK || OTHER_MODIFIER_CODES.has(code)) {
      // AltGr：系统先合成一个 左Ctrl 再报 右Alt（按住时连发也成对）—— 记成 右Alt，不当成 左Ctrl+右Alt。
      if (code === 'ControlLeft' && altGrSynthRef.current) return;
      if (code === 'AltRight' && pressedRef.current.has('ControlLeft')) {
        pressedRef.current.delete('ControlLeft');
        altGrSynthRef.current = true;
      }
      pressedRef.current.add(code);
      const p = modifierPreview();
      setPreview(p ? `${p}+` : '');
      return;
    }
    const key = mainKeyVk(code);
    if (key === undefined) return;
    const held = [...pressedRef.current];
    const sided = held.filter((c) => c in SIDED_MODIFIER_VK);
    const extra = held.some((c) => OTHER_MODIFIER_CODES.has(c));
    if (sided.length !== 1 || extra) {
      setCaptureError(t.shortcut_need_modifier);
      return;
    }
    setPending({ modifier: SIDED_MODIFIER_VK[sided[0]], key });
    setCaptureError(null);
    stopListening();
  };

  const handleKeyUp = (e: React.KeyboardEvent) => {
    e.preventDefault();
    e.stopPropagation();
    if (!listeningRef.current) return;
    pressedRef.current.delete(e.code);
    if (pressedRef.current.size === 0) altGrSynthRef.current = false;
    const p = modifierPreview();
    setPreview(p ? `${p}+` : '');
  };

  const error = captureError ?? shortcutError(pending, config, action, t);
  const canSave = !listening && !error && isValidBinding(pending) && !sameBinding(pending, current);

  const save = () => {
    if (!canSave) return;
    updateConfig({
      ...config,
      shortcuts: {
        ...DEFAULT_SHORTCUTS,
        ...(config.shortcuts ?? {}),
        [action]: pending,
      },
    });
    closeDialog();
  };

  const inactive = shortcutInactive(config, action);

  return (
    <>
      <span className="shortcut-inline">
        {t.shortcut_open}
        <button type="button" className="shortcut-link" onClick={openDialog}>
          {t.shortcut_link}
        </button>
        {t.shortcut_colon}
        <span className="shortcut-keys">{shortcutDisplay(current, t)}</span>
        {inactive && <span className="shortcut-inline-conflict">{t.shortcut_inline_conflict}</span>}
        {t.shortcut_close}
      </span>

      {open && (
        <div className="modal-overlay" onClick={closeDialog}>
          <div
            className="modal-dialog shortcut-dialog"
            role="dialog"
            aria-modal="true"
            aria-label={t.shortcut_dialog_title}
            onClick={(e) => e.stopPropagation()}
            onKeyDown={(e) => {
              if (!listeningRef.current && e.key === 'Escape') closeDialog();
            }}
          >
            <div className="modal-header">
              <svg className="modal-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <rect x="2" y="6" width="20" height="13" rx="2" />
                <path d="M6 10h.01M10 10h.01M14 10h.01M18 10h.01M8 14h8" />
              </svg>
              <span className="modal-title">{t.shortcut_dialog_title}</span>
              <button className="modal-close" onClick={closeDialog}>×</button>
            </div>
            <div className="modal-body">
              <div className="shortcut-dialog-feature">
                {t.shortcut_dialog_for}
                {t.shortcut_colon}
                <strong>{actionLabel(action, t)}</strong>
              </div>
              <div className="shortcut-dialog-caption">{t.shortcut_current}</div>
              <div className="hotkey-key-wrapper shortcut-dialog-key">
                {!listening ? (
                  <button type="button" className="hotkey-key-btn" onClick={startListening}>
                    {shortcutDisplay(pending, t)}
                  </button>
                ) : (
                  <div
                    className="hotkey-key-btn hotkey-key-listening"
                    tabIndex={0}
                    ref={keyRef}
                    role="textbox"
                    aria-label={t.shortcut_press_new}
                    onKeyDown={handleKeyDown}
                    onKeyUp={handleKeyUp}
                    onBlur={stopListening}
                  >
                    {preview || t.shortcut_press_new}
                  </div>
                )}
                <span className="hotkey-key-hint">{t.shortcut_rule_hint}</span>
              </div>
              {error && (
                <div className="shortcut-dialog-error" role="alert">
                  {error}
                </div>
              )}
            </div>
            <div className="modal-footer">
              <button type="button" className="btn btn-secondary" onClick={closeDialog}>
                {t.shortcut_cancel}
              </button>
              <button type="button" className="btn btn-primary" onClick={save} disabled={!canSave}>
                {t.shortcut_save}
              </button>
            </div>
          </div>
        </div>
      )}
    </>
  );
};

export default ShortcutSetting;
