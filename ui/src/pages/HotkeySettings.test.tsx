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

describe('HotkeySettingsPage - HOTKEY-078/079 AltGr 键序矩阵', () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'check_hotkey_available') return true;
      return null;
    });
  });

  // 翻译侧录制入口：先切到「翻译热键」子 Tab，再进入聆听态。
  function startTranslationRecording(container: HTMLElement): HTMLElement {
    fireEvent.click(screen.getByRole('button', { name: zhHans.hotkey_translation_tab }));
    const btn = container.querySelector<HTMLButtonElement>('.hotkey-key-btn');
    expect(btn).toBeTruthy();
    fireEvent.click(btn!);
    const listening = container.querySelector<HTMLElement>('.hotkey-key-listening');
    expect(listening).toBeTruthy();
    return listening!;
  }

  // HOTKEY-079-T1 · 翻译侧按 AltGr（keyDown ControlLeft → keyDown AltRight）→ 录成 Right Alt。
  // 生产路径：handleTranslationHotkeyKeyDown :369 ControlLeft 只挂 pending 不当场裁决
  //（此刻 getModifierState('AltGraph') 仍 false，当场判必错），:373 AltRight 到场 → finalize 0xA5。
  // 消融：删 :369/:373 两分支 → ControlLeft 走 :387 通用路径查 CODE_TO_VK['ControlLeft']=0xA2
  // → updateConfig 收到 0xA2，本条红。
  it('HOTKEY-079-T1: 翻译侧 AltGr 键序 → 录成 Right Alt 而非 Left Ctrl', async () => {
    const { container, updateConfig } = renderPage();
    const rec = startTranslationRecording(container);
    fireEvent.keyDown(rec, { code: 'ControlLeft' });
    fireEvent.keyDown(rec, { code: 'AltRight' });
    fireEvent.keyUp(rec, { code: 'AltRight' });
    await waitFor(() => expect(updateConfig).toHaveBeenCalled());
    expect(updateConfig.mock.calls[0][0].translation).toMatchObject({
      vk_code: 0xA5,
      display_name: 'Right Alt',
    });
    expect(screen.queryByRole('dialog')).toBeNull();
  });

  // HOTKEY-079-T2 · 翻译侧单按 Left Ctrl（keyDown + keyUp，中间无 AltRight）→ 仍录成 Left Ctrl。
  // 回归护栏：防 pending 吞键。生产路径：keyDown :369 pending=true 即 return（不 finalize）；
  // keyUp :408 pending 仍在 → finalize 0xA2（HOTKEY-079 的「延后裁决」由 keyUp 兜底）。
  // 消融：删 keyUp handler（或其内 finalize 分支）→ updateConfig 永不触发 → waitFor 红。
  it('HOTKEY-079-T2: 翻译侧单按 Left Ctrl → keyDown 后不定案，keyUp 后录成 Left Ctrl', async () => {
    const { container, updateConfig } = renderPage();
    const rec = startTranslationRecording(container);
    fireEvent.keyDown(rec, { code: 'ControlLeft' });
    expect(updateConfig).not.toHaveBeenCalled();
    fireEvent.keyUp(rec, { code: 'ControlLeft' });
    await waitFor(() => expect(updateConfig).toHaveBeenCalled());
    expect(updateConfig.mock.calls[0][0].translation).toMatchObject({
      vk_code: 0xA2,
      display_name: 'Left Ctrl',
    });
  });

  // HOTKEY-079-T3a · Escape 会话边界 pending 归零（泄漏探针，本单有判别力的消融）。
  // 真实泄漏路径（非人造事件序列）：用户按住 Left Ctrl 不放 → 按 Escape（或点走失焦，
  // onBlur :529 同样走 resetTranslationRecordingState）→ 重进录制态 → 才松手。
  // 这个 keyUp 落在没有对应 keyDown 的新会话里。生产契约：Escape 分支 :353 →
  // resetTranslationRecordingState :160 清 pending → 新会话里孤儿 keyUp ControlLeft
  // 因 pending=false 不 finalize，热键纹丝不动。
  // 消融：删 :160 清 pending 行 → 泄漏的 pending 让孤儿 keyUp 命中 :408 分支 →
  // finalize 0xA2 —— 用户在第二次会话里一个键都没按，热键被静默改成 Left Ctrl
  //（用户可见症状）→ updateConfig 被调用，本条红。
  // 注：coder-2 原规格序列（Escape→重进→单按 Right Ctrl）在删行消融下不变红
  //（ControlRight/AltRight 路径产出与 pending 真假无关，逐路径推演见 result.md），
  // 正向回归由 T3b 承担，泄漏探针由本条承担 —— 主控已独立复核并批准此拆分。
  it('HOTKEY-079-T3a: Escape 清 pending —— 重进录制后孤儿 keyUp ControlLeft 不得静默改热键', async () => {
    const { container, updateConfig } = renderPage();
    let rec = startTranslationRecording(container);
    fireEvent.keyDown(rec, { code: 'ControlLeft' });
    fireEvent.keyDown(rec, { code: 'Escape' });
    expect(container.querySelector('.hotkey-key-listening')).toBeNull();
    rec = startTranslationRecording(container);
    fireEvent.keyUp(rec, { code: 'ControlLeft' });
    expect(updateConfig).not.toHaveBeenCalled();
  });

  // HOTKEY-079-T3b · coder-2 原规格正向回归：Escape 终止会话 → 重进录制 →
  // 单按 Right Ctrl 正常录成 Right Ctrl（第二次会话功能完好，pending 无残留干扰）。
  // 生产路径：keyDown :359 ControlRight 查表 0xA3 → :387 通用 finalize。
  // 消融：删 :387 通用 finalize 路径（或 CODE_TO_VK 表 ControlRight 项）→ 红。
  // 注：本条在「删 :160 清 pending 行」消融下仍绿 —— 它守正向行为，不承担泄漏探针。
  it('HOTKEY-079-T3b: Escape 后重进录制，单按 Right Ctrl 正常录成 Right Ctrl', async () => {
    const { container, updateConfig } = renderPage();
    let rec = startTranslationRecording(container);
    fireEvent.keyDown(rec, { code: 'ControlLeft' });
    fireEvent.keyDown(rec, { code: 'Escape' });
    expect(container.querySelector('.hotkey-key-listening')).toBeNull();
    rec = startTranslationRecording(container);
    fireEvent.keyDown(rec, { code: 'ControlRight' });
    fireEvent.keyUp(rec, { code: 'ControlRight' });
    await waitFor(() => expect(updateConfig).toHaveBeenCalled());
    expect(updateConfig.mock.calls[0][0].translation).toMatchObject({
      vk_code: 0xA3,
      display_name: 'Right Ctrl',
    });
  });

  // HOTKEY-078-T4b · 语音侧 AltGr 键序矩阵：ControlLeft **先抬**。
  // T4a（AltRight 先抬）已由 HOTKEY-047-S1 覆盖（同键序同断言，冲突弹窗变体为 S12），
  // 矩阵成员保留即可，不重复造用例。
  // 序列：keyDown ControlLeft → keyDown AltRight → keyUp ControlLeft → keyUp AltRight(ctrlKey)。
  // 生产路径：AltRight keyDown :257-259 压掉合成 Ctrl 预览并亮旗 altGrSynthCtrlActive=true；
  // ControlLeft keyUp :297-300 旗在 → 只清 pressedMods 即 return（不定案、不清旗）；
  // AltRight keyUp :304 读旗 → :323 altGrSynthWasActive=true 压制 0x0002 → modifiers=0
  // → checkAndApplyVoiceHotkey(0xA5, 0)。
  // 消融（撤销 HOTKEY-078，把删掉的提前清旗三行加回）：**本用例不会红，这是结构性的**——
  // 三行的位置在 `const altGrSynthWasActive = ...` 捕获**之后**（见 git show 38cc809），
  // 而本用例键序 ControlLeft 先抬、AltRight keyUp 是末事件：捕获时旗仍为 true，
  // modifiers 照样是 0，其后再无 ControlLeft keyUp 让清旗产生可观测后果。
  // 078 的判别力由 **HOTKEY-047-S12** 承担（AltRight 先抬 + 尾随合成 Ctrl keyUp）：
  // 撤销三行后 S12 变红，弹窗显示 Left Ctrl，精确复现 078 原始症状 —— TEST-EXEC-081 实测。
  // 本用例守的是另一半矩阵：ControlLeft 先抬这条路径产出 0xA5/modifiers=0 且只 finalize 一次。
  it('HOTKEY-078-T4b: 语音侧 AltGr（ControlLeft 先抬）→ Right Alt modifiers=0', async () => {
    const { container, updateConfig } = renderPage();
    const rec = startRecording(container);
    fireEvent.keyDown(rec, { code: 'ControlLeft' });
    fireEvent.keyDown(rec, { code: 'AltRight' });
    fireEvent.keyUp(rec, { code: 'ControlLeft' });
    fireEvent.keyUp(rec, { code: 'AltRight', ctrlKey: true });
    await waitFor(() => expect(updateConfig).toHaveBeenCalled());
    expect(updateConfig.mock.calls[0][0].hotkey).toMatchObject({
      vk_code: 0xA5,
      modifiers: 0,
      display_name: 'Right Alt',
    });
    expect(updateConfig).toHaveBeenCalledTimes(1);
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
