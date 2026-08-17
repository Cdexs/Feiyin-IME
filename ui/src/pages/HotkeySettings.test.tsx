import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';

const mockInvoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: any[]) => mockInvoke(...args),
}));

vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => ({
    setMaximizable: vi.fn().mockResolvedValue(undefined),
    metadata: {},
  }),
}));

vi.mock('@tauri-apps/api/app', () => ({
  getVersion: vi.fn().mockResolvedValue('0.5.3'),
}));

import HotkeySettingsPage from './HotkeySettings.tsx';
import { voiceKeySet, translationKeySet, keysOverlap } from './HotkeySettings.tsx';
import { zhHans } from '../i18n/zh-Hans';
import { zhHant } from '../i18n/zh-Hant';
import { en } from '../i18n/en';

const baseConfig = {
  auto_start: false,
  hotkey: { mode: 'toggle', vk_code: 0x78, modifiers: 0 },
  ui_language: 'Chinese',
  translation: { enabled: false, vk_code: 0, display_name: '', target_language: 'Chinese' },
};

function renderPage(config = baseConfig) {
  const updateConfig = vi.fn();
  const utils = render(<HotkeySettingsPage config={config} updateConfig={updateConfig} />);
  return { ...utils, updateConfig };
}

function startRecording(container: HTMLElement): HTMLElement {
  const btn = container.querySelector<HTMLButtonElement>('.hotkey-key-btn');
  expect(btn).toBeTruthy();
  fireEvent.click(btn!);
  const listening = container.querySelector<HTMLElement>('.hotkey-key-listening');
  expect(listening).toBeTruthy();
  return listening!;
}

describe('HotkeySettingsPage - HOTKEY-047', () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'check_hotkey_available') return true;
      return null;
    });
  });

  it('HOTKEY-047-S1: 右 Alt 单键 → vk=0xA5 modifiers=0 display=Right Alt', async () => {
    const { container, updateConfig } = renderPage();
    const rec = startRecording(container);
    fireEvent.keyDown(rec, { code: 'ControlLeft' });
    fireEvent.keyDown(rec, { code: 'AltRight' });
    fireEvent.keyUp(rec, { code: 'AltRight', ctrlKey: true });
    fireEvent.keyUp(rec, { code: 'ControlLeft' });
    await waitFor(() => expect(updateConfig).toHaveBeenCalled());
    expect(updateConfig.mock.calls[0][0].hotkey).toMatchObject({
      vk_code: 0xA5,
      modifiers: 0,
      display_name: 'Right Alt',
    });
  });

  it('HOTKEY-047-S2: AltGr 组合（右 Alt+M）→ vk=0x4D modifiers=0x0001 display=Alt+M', async () => {
    const { container, updateConfig } = renderPage();
    const rec = startRecording(container);
    fireEvent.keyDown(rec, { code: 'ControlLeft' });
    fireEvent.keyDown(rec, { code: 'AltRight' });
    fireEvent.keyDown(rec, { code: 'KeyM', altKey: true, ctrlKey: true });
    await waitFor(() => expect(updateConfig).toHaveBeenCalled());
    expect(updateConfig.mock.calls[0][0].hotkey).toMatchObject({
      vk_code: 0x4D,
      modifiers: 0x0001,
      display_name: 'Alt+M',
    });
  });

  it('HOTKEY-047-S3: 左 Alt 单键 → vk=0xA4 modifiers=0 display=Left Alt', async () => {
    const { container, updateConfig } = renderPage();
    const rec = startRecording(container);
    fireEvent.keyDown(rec, { code: 'AltLeft' });
    fireEvent.keyUp(rec, { code: 'AltLeft' });
    await waitFor(() => expect(updateConfig).toHaveBeenCalled());
    expect(updateConfig.mock.calls[0][0].hotkey).toMatchObject({
      vk_code: 0xA4,
      modifiers: 0,
      display_name: 'Left Alt',
    });
  });

  it('HOTKEY-047-S4: 左 Ctrl 单键 → vk=0xA2 modifiers=0 display=Left Ctrl', async () => {
    const { container, updateConfig } = renderPage();
    const rec = startRecording(container);
    fireEvent.keyDown(rec, { code: 'ControlLeft' });
    fireEvent.keyUp(rec, { code: 'ControlLeft' });
    await waitFor(() => expect(updateConfig).toHaveBeenCalled());
    expect(updateConfig.mock.calls[0][0].hotkey).toMatchObject({
      vk_code: 0xA2,
      modifiers: 0,
      display_name: 'Left Ctrl',
    });
  });

  it('HOTKEY-047-S5: 右 Ctrl 单键 → vk=0xA3 modifiers=0 display=Right Ctrl', async () => {
    const { container, updateConfig } = renderPage();
    const rec = startRecording(container);
    fireEvent.keyDown(rec, { code: 'ControlRight' });
    fireEvent.keyUp(rec, { code: 'ControlRight' });
    await waitFor(() => expect(updateConfig).toHaveBeenCalled());
    expect(updateConfig.mock.calls[0][0].hotkey).toMatchObject({
      vk_code: 0xA3,
      modifiers: 0,
      display_name: 'Right Ctrl',
    });
  });

  it('HOTKEY-047-S6: 左 Shift 单键 → vk=0xA0 modifiers=0 display=Left Shift', async () => {
    const { container, updateConfig } = renderPage();
    const rec = startRecording(container);
    fireEvent.keyDown(rec, { code: 'ShiftLeft' });
    fireEvent.keyUp(rec, { code: 'ShiftLeft' });
    await waitFor(() => expect(updateConfig).toHaveBeenCalled());
    expect(updateConfig.mock.calls[0][0].hotkey).toMatchObject({
      vk_code: 0xA0,
      modifiers: 0,
      display_name: 'Left Shift',
    });
  });

  it('HOTKEY-047-S7: Ctrl+Shift+M → vk=0x4D modifiers=0x0006 display=Ctrl+Shift+M', async () => {
    const { container, updateConfig } = renderPage();
    const rec = startRecording(container);
    fireEvent.keyDown(rec, { code: 'ControlLeft' });
    fireEvent.keyDown(rec, { code: 'ShiftLeft' });
    fireEvent.keyDown(rec, { code: 'KeyM', ctrlKey: true, shiftKey: true });
    await waitFor(() => expect(updateConfig).toHaveBeenCalled());
    expect(updateConfig.mock.calls[0][0].hotkey).toMatchObject({
      vk_code: 0x4D,
      modifiers: 0x0006,
      display_name: 'Ctrl+Shift+M',
    });
  });

  it('HOTKEY-047-S8: finalizedRef 防重入护栏 —— 组合键定案后释放修饰键不覆盖', async () => {
    const { container, updateConfig } = renderPage();
    const rec = startRecording(container);
    fireEvent.keyDown(rec, { code: 'ControlLeft' });
    fireEvent.keyDown(rec, { code: 'ShiftLeft' });
    fireEvent.keyDown(rec, { code: 'KeyM', ctrlKey: true, shiftKey: true });
    fireEvent.keyUp(rec, { code: 'ShiftLeft' });
    fireEvent.keyUp(rec, { code: 'ControlLeft' });
    await waitFor(() => expect(updateConfig).toHaveBeenCalledTimes(1));
    expect(updateConfig.mock.calls[0][0].hotkey).toMatchObject({
      vk_code: 0x4D,
      modifiers: 0x0006,
      display_name: 'Ctrl+Shift+M',
    });
  });

  it('HOTKEY-047-S9: 点击进入录制态后录制框同步成为 document.activeElement', () => {
    const { container } = renderPage();
    const btn = container.querySelector<HTMLButtonElement>('.hotkey-key-btn');
    fireEvent.click(btn!);
    const listening = container.querySelector<HTMLElement>('.hotkey-key-listening');
    expect(listening).toBeTruthy();
    expect(document.activeElement).toBe(listening);
  });

  it('HOTKEY-047-S10: 按下左 Alt 时录制框预览立即变为 Left Alt', () => {
    const { container } = renderPage();
    const rec = startRecording(container);
    expect(rec.textContent).toBe(zhHans.hotkey_press_new);
    fireEvent.keyDown(rec, { code: 'AltLeft' });
    expect(rec.textContent).toBe('Left Alt');
  });

  it('HOTKEY-047-S11: Escape 退出录制态且 updateConfig 零调用', () => {
    const { container, updateConfig } = renderPage();
    const rec = startRecording(container);
    fireEvent.keyDown(rec, { code: 'Escape' });
    expect(container.querySelector('.hotkey-key-listening')).toBeNull();
    expect(container.querySelector('.hotkey-key-btn')).toBeTruthy();
    expect(updateConfig).not.toHaveBeenCalled();
  });

  it('HOTKEY-047-S12: 冲突走查（右 Alt 路径）→ 弹冲突弹窗而非 updateConfig', async () => {
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'check_hotkey_available') return false;
      return null;
    });
    const { container, updateConfig } = renderPage();
    const rec = startRecording(container);
    fireEvent.keyDown(rec, { code: 'ControlLeft' });
    fireEvent.keyDown(rec, { code: 'AltRight' });
    fireEvent.keyUp(rec, { code: 'AltRight', ctrlKey: true });
    fireEvent.keyUp(rec, { code: 'ControlLeft' });
    await waitFor(() => expect(screen.getByRole('dialog')).toBeInTheDocument());
    expect(updateConfig).not.toHaveBeenCalled();
    expect(screen.getByText(/Right Alt/)).toBeInTheDocument();
    expect(screen.getByText(zhHans.hotkey_conflict)).toBeInTheDocument();
  });

  it('HOTKEY-047-LOC: hotkey_click_to_change 三份 locale 均存在且非空', () => {
    expect(zhHans.hotkey_click_to_change).toBeTruthy();
    expect(zhHant.hotkey_click_to_change).toBeTruthy();
    expect(en.hotkey_click_to_change).toBeTruthy();
  });
});

describe('HotkeySettingsPage - HOTKEY-049 纯函数护栏', () => {
  const toSortedArray = (s: Set<number>) => [...s].sort((a, b) => a - b);

  // Gavin 拍板的七条实例对照表（逐条一个用例）
  it('HOTKEY-049-T1: 语音=右 Alt(165,0)，翻译=右 Alt(0xA5) → 拦（交集 {0xA5}）', () => {
    const vSet = voiceKeySet(165, 0);
    expect(toSortedArray(vSet)).toEqual([0xA5]);
    const tSet = translationKeySet(0xA5);
    expect(keysOverlap(vSet, tSet)).toBe(true);
  });

  it('HOTKEY-049-T2: 语音=右 Alt(165,0)，翻译=左 Alt(0xA4) → 放行（空交集）', () => {
    const vSet = voiceKeySet(165, 0);
    const tSet = translationKeySet(0xA4);
    expect(keysOverlap(vSet, tSet)).toBe(false);
  });

  it('HOTKEY-049-T3: 语音=Alt+M(0x4D,0x1)，翻译=右 Alt(0xA5) → 拦', () => {
    const vSet = voiceKeySet(0x4D, 0x1);
    expect(toSortedArray(vSet)).toEqual([0x4D, 0xA4, 0xA5]);
    const tSet = translationKeySet(0xA5);
    expect(keysOverlap(vSet, tSet)).toBe(true);
  });

  it('HOTKEY-049-T4: 语音=Alt+M(0x4D,0x1)，翻译=左 Alt(0xA4) → 拦（左右都算）', () => {
    const vSet = voiceKeySet(0x4D, 0x1);
    const tSet = translationKeySet(0xA4);
    expect(keysOverlap(vSet, tSet)).toBe(true);
  });

  it('HOTKEY-049-T5: 语音=Alt+M(0x4D,0x1)，翻译=右 Ctrl(0xA3) → 放行', () => {
    const vSet = voiceKeySet(0x4D, 0x1);
    const tSet = translationKeySet(0xA3);
    expect(keysOverlap(vSet, tSet)).toBe(false);
  });

  it('HOTKEY-049-T6: 语音=Ctrl+Shift+M(0x4D,0x6)，翻译=左 Ctrl(0xA2) → 拦', () => {
    const vSet = voiceKeySet(0x4D, 0x6);
    expect(toSortedArray(vSet)).toEqual([0x4D, 0xA0, 0xA1, 0xA2, 0xA3]);
    const tSet = translationKeySet(0xA2);
    expect(keysOverlap(vSet, tSet)).toBe(true);
  });

  it('HOTKEY-049-T7: 语音=Ctrl+Shift+M(0x4D,0x6)，翻译=右 Alt(0xA5) → 放行', () => {
    const vSet = voiceKeySet(0x4D, 0x6);
    const tSet = translationKeySet(0xA5);
    expect(keysOverlap(vSet, tSet)).toBe(false);
  });

  // 追加边界用例
  it('HOTKEY-049-T8: translationKeySet(0)（翻译未设置）→ 空集，keysOverlap 恒 false（不得拦）', () => {
    const tSet = translationKeySet(0);
    expect(tSet.size).toBe(0);
    const vSet = voiceKeySet(0xA5, 0);
    expect(keysOverlap(vSet, tSet)).toBe(false);
    expect(keysOverlap(tSet, vSet)).toBe(false);
  });

  it('HOTKEY-049-T9: 修饰键展开穷举 —— MOD_ALT→{0xA4,0xA5} / MOD_CONTROL→{0xA2,0xA3} / MOD_SHIFT→{0xA0,0xA1} / MOD_WIN→{0x5B,0x5C}', () => {
    const cases: Array<[number, number[]]> = [
      [0x0001, [0xA4, 0xA5]],
      [0x0002, [0xA2, 0xA3]],
      [0x0004, [0xA0, 0xA1]],
      [0x0008, [0x5B, 0x5C]],
    ];
    for (const [mod, expectedMods] of cases) {
      const set = voiceKeySet(0x41, mod);
      for (const m of expectedMods) expect(set.has(m)).toBe(true);
    }
  });

  it('HOTKEY-049-T10: keysOverlap 空集短路 —— 任一侧为空 → false', () => {
    expect(keysOverlap(new Set(), new Set([0xA5]))).toBe(false);
    expect(keysOverlap(new Set([0xA5]), new Set())).toBe(false);
    expect(keysOverlap(new Set(), new Set())).toBe(false);
  });

  it('HOTKEY-049-T11: 组合修饰键 mod=0x7（Ctrl+Alt+Shift）→ 六个修饰键 vk 全在集合内', () => {
    const set = voiceKeySet(0x4D, 0x7);
    const allModKeys = [0xA2, 0xA3, 0xA4, 0xA5, 0xA0, 0xA1];
    for (const vk of allModKeys) expect(set.has(vk)).toBe(true);
    expect(set.has(0x4D)).toBe(true);
  });
});
