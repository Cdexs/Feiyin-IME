import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor, cleanup } from '@testing-library/react';

// Mocks must be before imports
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

import VoicePage from './Voice.tsx';
// @ts-ignore - i18n
import { zhHans } from '../i18n/zh-Hans';

describe('VoicePage - PUNCT-UI-001', () => {
  const baseConfig = {
    auto_start: false,
    hotkey: { mode: 'toggle' as const, vk_code: 0x78, modifiers: 0 },
    ui_language: 'Chinese' as const,
    audio: {
      device: '',
      silence_threshold: 0.01,
      silence_duration_ms: 1500,
      asr_model: 'performance',
      qwen3_asr_url: 'wss://dashscope.aliyuncs.com/api-qwen3-asr/v1/realtime',
      qwen3_asr_model: 'qwen3-asr-flash-realtime',
      asr_online_api_key: '',
    },
    llm: { enabled: false, api_url: '', api_key: '', model: '' },
    wordbook: [] as string[],
    overlay_opacity: 1.0,
    transcription_language: 'zh',
  };

  beforeEach(() => {
    mockInvoke.mockReset();
    mockInvoke.mockImplementation(async (cmd: string, _args?: any) => {
      if (cmd === 'get_config') return baseConfig;
      if (cmd === 'save_config') return true;
      if (cmd === 'test_qwen3_asr_connection') {
        const apiKey = _args?.apiKey || '';
        if (apiKey === 'sk-test-key-123456') return 'OK';
        throw new Error('Invalid key');
      }
      if (cmd === 'check_accuracy_model_ready') return true;
      return null;
    });
  });

  // PUNCT-UI-001 through PUNCT-UI-007: punctuation toggle tests
  describe('PUNCT-UI-001', () => {
    it('PUNCT-UI-001: renders punctuation toggle checkbox', async () => {
      render(<VoicePage config={baseConfig} updateConfig={vi.fn()} />);
      await waitFor(() => {
        expect(screen.getByText(zhHans.voice_auto_punctuation as string)).toBeInTheDocument();
      });
    });

    it('PUNCT-UI-002: punctuation checkbox reflects config.audio.auto_punctuation', async () => {
      const config = {
        ...baseConfig,
        audio: { ...baseConfig.audio, auto_punctuation: true },
      };
      render(<VoicePage config={config} updateConfig={vi.fn()} />);
      await waitFor(() => {
        const checkboxes = screen.getAllByRole('checkbox');
        // V091-PUNCT-TAIL-214: 识别输出区现有两个开关（自动补全标点 / 句尾不显示标点符号）
        expect(checkboxes.length).toBe(2);
        const checkbox = checkboxes[0] as HTMLInputElement;
        expect(checkbox.checked).toBe(true);
      });
    });

    it('PUNCT-UI-003: clicking punctuation checkbox toggles to false', async () => {
      const updateConfig = vi.fn();
      const config = {
        ...baseConfig,
        audio: { ...baseConfig.audio, auto_punctuation: true },
      };
      render(<VoicePage config={config} updateConfig={updateConfig} />);
      await waitFor(() => {
        const checkboxes = screen.getAllByRole('checkbox');
        const checkbox = checkboxes[0];
        fireEvent.click(checkbox);
        expect(updateConfig).toHaveBeenCalled();
      });
    });

    it('PUNCT-UI-004: auto_punctuation label text is rendered', async () => {
      render(<VoicePage config={baseConfig} updateConfig={vi.fn()} />);
      await waitFor(() => {
        expect(
          screen.getByText(zhHans.voice_auto_punctuation as string)
        ).toBeInTheDocument();
      });
    });

    it('PUNCT-UI-005: section title renders', async () => {
      render(<VoicePage config={baseConfig} updateConfig={vi.fn()} />);
      await waitFor(() => {
        expect(
          screen.getByText(zhHans.voice_recognition_output as string)
        ).toBeInTheDocument();
      });
    });

    it('PUNCT-UI-006: checkbox label is clickable', async () => {
      render(<VoicePage config={baseConfig} updateConfig={vi.fn()} />);
      await waitFor(() => {
        const checkbox = screen.getAllByRole('checkbox')[0];
        expect(checkbox).not.toBeDisabled();
      });
    });

    it('PUNCT-UI-007: checkbox toggles on click when unchecked', async () => {
      const updateConfig = vi.fn();
      render(<VoicePage config={baseConfig} updateConfig={updateConfig} />);
      await waitFor(() => {
        const checkbox = screen.getAllByRole('checkbox')[0];
        fireEvent.click(checkbox);
        expect(updateConfig).toHaveBeenCalled();
      });
    });
  });

  describe('ASR-UI-001', () => {
    it('ASR-UI-001: renders ASR model select element', async () => {
      render(<VoicePage config={baseConfig} updateConfig={vi.fn()} />);
      await waitFor(() => {
        expect(
          screen.getByText(zhHans.voice_asr_model as string)
        ).toBeInTheDocument();
        const select = screen.getAllByRole('combobox')[1];
        expect(select).toBeInTheDocument();
      });
    });

    // ASR-UI-208（Gavin 2026-09-10）：UI 移除「在线语音识别模型」(qwen_audio_online) 选项
    it('ASR-UI-002: select has two options (performance / fun-asr)', async () => {
      render(<VoicePage config={baseConfig} updateConfig={vi.fn()} />);
      await waitFor(() => {
        const select = screen.getAllByRole('combobox')[1];
        expect(select).toBeInTheDocument();
        const options = select.querySelectorAll('option');
        expect(options.length).toBe(2);
      });
    });

    it('ASR-UI-003: default selected value is "performance"', async () => {
      render(<VoicePage config={baseConfig} updateConfig={vi.fn()} />);
      await waitFor(() => {
        const select = screen.getAllByRole('combobox')[1] as HTMLSelectElement;
        expect(select.value).toBe('performance');
      });
    });

    it('ASR-UI-004: options display correct i18n text', async () => {
      render(<VoicePage config={baseConfig} updateConfig={vi.fn()} />);
      await waitFor(() => {
        expect(
          screen.getByText(zhHans.voice_asr_model_performance as string)
        ).toBeInTheDocument();
        // ASR-UI-208：qwen 选项已移除，其文案不应再出现
        expect(
          screen.queryByText(zhHans.voice_asr_model_qwen3 as string)
        ).not.toBeInTheDocument();
        expect(
          screen.queryByText(zhHans.voice_asr_model_accuracy as string)
        ).not.toBeInTheDocument();
      });
    });

    it('ASR-UI-005: changing select updates config', async () => {
      const updateConfig = vi.fn();
      render(<VoicePage config={baseConfig} updateConfig={updateConfig} />);
      await waitFor(() => {
        const select = screen.getAllByRole('combobox')[1];
        // ASR-UI-208：qwen 已不可选，改用 fun_asr_realtime 验证 select→config 通路
        fireEvent.change(select, { target: { value: 'fun_asr_realtime' } });
        expect(updateConfig).toHaveBeenCalledWith(
          expect.objectContaining({
            audio: expect.objectContaining({ asr_model: 'fun_asr_realtime' }),
          })
        );
      });
    });

    it('ASR-UI-006: description text uses brand color', async () => {
      render(<VoicePage config={baseConfig} updateConfig={vi.fn()} />);
      await waitFor(() => {
        const desc = screen.getByText(zhHans.voice_asr_model_performance_desc as string);
        expect(desc).toBeInTheDocument();
        expect(desc.className).toContain('asr-model-desc');
      });
    });

    it('ASR-UI-007: description shows correct text for performance model', async () => {
      render(<VoicePage config={baseConfig} updateConfig={vi.fn()} />);
      await waitFor(() => {
        expect(screen.getByText(zhHans.voice_asr_model_performance_desc as string)).toBeInTheDocument();
      });
    });

    // 🔴 ASR-UI-208（Gavin 2026-09-10）：以下用例全部通过 <select> 切到
    // qwen_audio_online 再断言其专属区块（API key 输入 / 测试连接 / 切走隐藏 / unmount 回落）。
    // UI 已移除该选项，且显示层对存量该值回落到 performance ⇒ 这些路径**不可达**，
    // 非行为回归。故 skip 而非删除：后端能力与配置字段均保留不动，
    // 将来若恢复该入口，本组用例可原样启用。
    it.skip('ASR-UI-008: Qwen3 model shows API key input', async () => {
      const qwenConfig = {...baseConfig, audio: {...baseConfig.audio, asr_model: 'qwen_audio_online'}};
      render(<VoicePage config={qwenConfig} updateConfig={vi.fn()} />);
      await waitFor(() => {
        expect(screen.getByText(zhHans.voice_asr_online_api_key as string)).toBeInTheDocument();
      });
    });

    it.skip('ASR-UI-009: Qwen3 model shows test connection button', async () => {
      const qwenConfig = {...baseConfig, audio: {...baseConfig.audio, asr_model: 'qwen_audio_online'}};
      render(<VoicePage config={qwenConfig} updateConfig={vi.fn()} />);
      await waitFor(() => {
        expect(screen.getByText(zhHans.voice_qwen3_test_connection as string)).toBeInTheDocument();
      });
    });

    it.skip('ASR-UI-010: Qwen3 test connection button disabled when no key', async () => {
      const emptyKeyConfig = {...baseConfig, audio: {...baseConfig.audio, asr_model: 'qwen_audio_online', asr_online_api_key: ''}};
      render(<VoicePage config={emptyKeyConfig} updateConfig={vi.fn()} />);
      await waitFor(() => {
        const btn = screen.getByText(zhHans.voice_qwen3_test_connection as string).closest('button');
        expect(btn).toBeDisabled();
      });
    });

    it.skip('ASR-UI-011: Qwen3 test connection button enabled with key', async () => {
      const hasKeyConfig = {...baseConfig, audio: {...baseConfig.audio, asr_model: 'qwen_audio_online', asr_online_api_key: 'sk-test-key'}};
      render(<VoicePage config={hasKeyConfig} updateConfig={vi.fn()} />);
      await waitFor(() => {
        const btn = screen.getByText(zhHans.voice_qwen3_test_connection as string).closest('button');
        expect(btn).not.toBeDisabled();
      });
    });

    it.skip('ASR-UI-012: clicking test connection shows testing state', async () => {
      // Override mock with delay for this test
      mockInvoke.mockImplementation(async (cmd: string) => {
        if (cmd === 'get_config') return baseConfig;
        if (cmd === 'test_qwen3_asr_connection') await new Promise(r => setTimeout(r, 500));
        if (cmd === 'check_accuracy_model_ready') return true;
        return null;
      });
      const goodKeyConfig = {...baseConfig, audio: {...baseConfig.audio, asr_model: 'qwen_audio_online', asr_online_api_key: 'sk-test-key-123456'}};
      render(<VoicePage config={goodKeyConfig} updateConfig={vi.fn()} />);
      await waitFor(() => {
        const btn = screen.getByText(zhHans.voice_qwen3_test_connection as string).closest('button');
        fireEvent.click(btn!);
      });
      await waitFor(() => {
        expect(screen.getByText(zhHans.voice_qwen3_testing as string)).toBeInTheDocument();
      });
    });

    it.skip('ASR-UI-013: successful connection shows success message', async () => {
      const goodKeyConfig = {...baseConfig, audio: {...baseConfig.audio, asr_model: 'qwen_audio_online', asr_online_api_key: 'sk-test-key-123456'}};
      render(<VoicePage config={goodKeyConfig} updateConfig={vi.fn()} />);
      await waitFor(() => {
        const btn = screen.getByText(zhHans.voice_qwen3_test_connection as string).closest('button');
        fireEvent.click(btn!);
      });
      await waitFor(() => {
        expect(screen.getByText(zhHans.voice_qwen3_test_success as string)).toBeInTheDocument();
      });
    });

    it.skip('ASR-UI-014: failed connection shows failure message', async () => {
      const badKeyConfig = {...baseConfig, audio: {...baseConfig.audio, asr_model: 'qwen_audio_online', asr_online_api_key: 'bad-key'}};
      render(<VoicePage config={badKeyConfig} updateConfig={vi.fn()} />);
      await waitFor(() => {
        const btn = screen.getByText(zhHans.voice_qwen3_test_connection as string).closest('button');
        fireEvent.click(btn!);
      });
      await waitFor(() => {
        expect(screen.getByText(zhHans.voice_qwen3_test_failed as string)).toBeInTheDocument();
      });
    });

    it.skip('ASR-UI-015: switching models hides Qwen3 section', async () => {
      const { rerender } = render(<VoicePage config={baseConfig} updateConfig={vi.fn()} />);
      const qwenConfig = {...baseConfig, audio: {...baseConfig.audio, asr_model: 'qwen_audio_online'}};
      rerender(<VoicePage config={qwenConfig} updateConfig={vi.fn()} />);
      await waitFor(() => {
        expect(screen.getByText(zhHans.voice_asr_online_api_key as string)).toBeInTheDocument();
      });
      rerender(<VoicePage config={baseConfig} updateConfig={vi.fn()} />);
      await waitFor(() => {
        expect(screen.queryByText(zhHans.voice_asr_online_api_key as string)).not.toBeInTheDocument();
      });
    });

    it.skip('ASR-UI-016: empty key hint shown when switching to Qwen3 without key', async () => {
      const qwenConfig = {...baseConfig, audio: {...baseConfig.audio, asr_model: 'qwen_audio_online', asr_online_api_key: ''}};
      render(<VoicePage config={qwenConfig} updateConfig={vi.fn()} />);
      await waitFor(() => {
        expect(screen.getByText(zhHans.voice_qwen3_empty_key_hint as string)).toBeInTheDocument();
      });
    });
    // R2 fallback tests — use wrapper to simulate real parent state management
    it.skip('FALLBACK-001: switch to qwen3(no key) then unmount falls back to performance', async () => {
      let currentConfig = JSON.parse(JSON.stringify(baseConfig));
      const updateFn = vi.fn((cfg: any) => { currentConfig = cfg; });
      const { rerender, unmount } = render(<VoicePage config={currentConfig} updateConfig={updateFn} />);
      // Simulate user selecting qwen_audio_online via dropdown
      const select = screen.getAllByRole('combobox')[1];
      fireEvent.change(select, { target: { value: 'qwen_audio_online' } });
      // updateFn was called with new config; re-render as parent would
      expect(updateFn).toHaveBeenCalled();
      const qwenConfig = {...baseConfig, audio: {...baseConfig.audio, asr_model: 'qwen_audio_online'}};
      rerender(<VoicePage config={qwenConfig} updateConfig={updateFn} />);
      // Wait for latestRef to update
      await new Promise(r => setTimeout(r, 50));
      // Now unmount — cleanup should detect qwen3+no-key and fallback
      unmount();
      // updateFn should have been called at least twice (one for model change, one for fallback)
      const calls = updateFn.mock.calls.map((call: any[]) => call[0]);
      const fallbackCalls = calls.filter((cfg: any) =>
        cfg?.audio?.asr_model === 'performance'
      );
      expect(fallbackCalls.length).toBeGreaterThanOrEqual(1);
    });

    it.skip('FALLBACK-002: switch to qwen3(with key) then unmount does NOT fallback', async () => {
      let currentConfig = JSON.parse(JSON.stringify(baseConfig));
      const updateFn = vi.fn((cfg: any) => { currentConfig = cfg; });
      const { rerender, unmount } = render(<VoicePage config={currentConfig} updateConfig={updateFn} />);
      const select = screen.getAllByRole('combobox')[1];
      fireEvent.change(select, { target: { value: 'qwen_audio_online' } });
      // Re-render with key set
      const qwenKeyConfig = {...baseConfig, audio: {...baseConfig.audio, asr_model: 'qwen_audio_online', asr_online_api_key: 'sk-test-key'}};
      rerender(<VoicePage config={qwenKeyConfig} updateConfig={updateFn} />);
      await new Promise(r => setTimeout(r, 50));
      unmount();
      // 意图：with key 时 cleanup 不回退。注意不能按 "performance 调用数=0" 断言——
      // handleAsrModelChange 连续两次 handleAudioChange（陈旧 config 重建）会产生一条
      // 中间调用（asr_online_model 已同步但 asr_model 仍是旧值），污染过滤。
      // 忠实断言：最后一个 updateConfig 调用必须仍保持在线族（未回退）。
      const calls = updateFn.mock.calls.map((call: any[]) => call[0]);
      const lastConfig = calls[calls.length - 1];
      expect(lastConfig.audio?.asr_model).toBe('qwen_audio_online');
    });

    it('FALLBACK-003: mount with qwen3+key does not fallback on unmount', async () => {
      const updateConfig = vi.fn();
      const qwenHasKeyConfig = {...baseConfig, audio: {...baseConfig.audio, asr_model: 'qwen_audio_online', asr_online_api_key: 'sk-test-key-123456'}};
      const { unmount } = render(<VoicePage config={qwenHasKeyConfig} updateConfig={updateConfig} />);
      await new Promise(r => setTimeout(r, 50));
      unmount();
      expect(updateConfig).not.toHaveBeenCalled();
    });

    // ---- TEST-SYNC-056: fun_asr_realtime UI 仿写 ----
    it('FUN-UI-001: fun_asr_realtime appears in select options', async () => {
      render(<VoicePage config={baseConfig} updateConfig={vi.fn()} />);
      await waitFor(() => {
        const options = screen.getAllByRole('combobox')[1].querySelectorAll('option');
        const values = Array.from(options).map((o) => (o as HTMLOptionElement).value);
        expect(values).toContain('fun_asr_realtime');
        // ASR-UI-208：qwen 选项已移除
        expect(values).not.toContain('qwen_audio_online');
      });
    });

    it('FUN-UI-002: fun_asr_realtime shows its description text', async () => {
      const funConfig = {...baseConfig, audio: {...baseConfig.audio, asr_model: 'fun_asr_realtime'}};
      render(<VoicePage config={funConfig} updateConfig={vi.fn()} />);
      await waitFor(() => {
        expect(screen.getByText(zhHans.voice_asr_model_fun_asr_desc as string)).toBeInTheDocument();
      });
    });

    it('FUN-UI-003: fun_asr_realtime shows API key input', async () => {
      const funConfig = {...baseConfig, audio: {...baseConfig.audio, asr_model: 'fun_asr_realtime'}};
      render(<VoicePage config={funConfig} updateConfig={vi.fn()} />);
      await waitFor(() => {
        expect(screen.getByText(zhHans.voice_asr_online_api_key as string)).toBeInTheDocument();
      });
    });

    // 忠实合同断言（预计对当前生产红，见 TEST-SYNC-056 result.md 缺陷上报）
    // handleAsrModelChange 先 handleAudioChange('asr_online_model','fun-asr-realtime')
    // 再 handleAudioChange('asr_model',value)，两次都从同一 render 的陈旧 config 重建，
    // App.tsx updateConfig 是整体替换 → 第二次调用覆盖第一次，asr_online_model 同步实际丢失。
    it('FUN-UI-004: switching to fun_asr_realtime persists BOTH asr_model and synced asr_online_model', async () => {
      let currentConfig = JSON.parse(JSON.stringify(baseConfig));
      const updateFn = vi.fn((cfg: any) => { currentConfig = cfg; });
      const { rerender } = render(<VoicePage config={currentConfig} updateConfig={updateFn} />);
      const select = screen.getAllByRole('combobox')[1];
      fireEvent.change(select, { target: { value: 'fun_asr_realtime' } });
      rerender(<VoicePage config={currentConfig} updateConfig={updateFn} />);
      await new Promise(r => setTimeout(r, 50));
      const calls = updateFn.mock.calls.map((call: any[]) => call[0]);
      const finalConfig = calls[calls.length - 1];
      // 意图：切换后 asr_model 与 asr_online_model 同时生效（064 注释称主动同步避免守卫 warn）
      expect(finalConfig.audio?.asr_model).toBe('fun_asr_realtime');
      expect(finalConfig.audio?.asr_online_model).toBe('fun-asr-realtime');
    });

    it('FUNFALLBACK-001: switch to fun_asr(no key) then unmount falls back to performance', async () => {
      let currentConfig = JSON.parse(JSON.stringify(baseConfig));
      const updateFn = vi.fn((cfg: any) => { currentConfig = cfg; });
      const { rerender, unmount } = render(<VoicePage config={currentConfig} updateConfig={updateFn} />);
      const select = screen.getAllByRole('combobox')[1];
      fireEvent.change(select, { target: { value: 'fun_asr_realtime' } });
      const funConfig = {...baseConfig, audio: {...baseConfig.audio, asr_model: 'fun_asr_realtime'}};
      rerender(<VoicePage config={funConfig} updateConfig={updateFn} />);
      await new Promise(r => setTimeout(r, 50));
      unmount();
      const calls = updateFn.mock.calls.map((call: any[]) => call[0]);
      const fallbackCalls = calls.filter((cfg: any) => cfg?.audio?.asr_model === 'performance');
      expect(fallbackCalls.length).toBeGreaterThanOrEqual(1);
    });

    it('FUNFALLBACK-002: switch to fun_asr(with key) then unmount does NOT fallback', async () => {
      let currentConfig = JSON.parse(JSON.stringify(baseConfig));
      const updateFn = vi.fn((cfg: any) => { currentConfig = cfg; });
      const { rerender, unmount } = render(<VoicePage config={currentConfig} updateConfig={updateFn} />);
      const select = screen.getAllByRole('combobox')[1];
      fireEvent.change(select, { target: { value: 'fun_asr_realtime' } });
      const funKeyConfig = {...baseConfig, audio: {...baseConfig.audio, asr_model: 'fun_asr_realtime', asr_online_api_key: 'sk-test-key'}};
      rerender(<VoicePage config={funKeyConfig} updateConfig={updateFn} />);
      await new Promise(r => setTimeout(r, 50));
      unmount();
      // 意图：with key 时 cleanup 不回退（同上，按最后一个调用断言避免中间调用污染）。
      const calls = updateFn.mock.calls.map((call: any[]) => call[0]);
      const lastConfig = calls[calls.length - 1];
      expect(lastConfig.audio?.asr_model).toBe('fun_asr_realtime');
    });

  });

  // ==========================================================================
  // V091-PUNCT-TAIL-214 · 阶段三交叉护栏（TEST-SYNC-214，coder-2 独立推导）
  // --------------------------------------------------------------------------
  // 契约层：新开关存在 / 默认关 / 绑到 punctuation.strip_trailing / 三语 locale 齐全。
  // 刻意不做的：颜色、布局、圆角等渲染细节（属 Browser Mode 面，与验收标准 a 无关）。
  // ==========================================================================
  describe('V091-PUNCT-TAIL-214', () => {
    it('PUNCT-214-UI-001: renders trailing-punctuation label + hint', async () => {
      render(<VoicePage config={baseConfig} updateConfig={vi.fn()} />);
      await waitFor(() => {
        expect(
          screen.getByText(zhHans.voice_strip_trailing_punct as string)
        ).toBeInTheDocument();
        expect(
          screen.getByText(zhHans.voice_strip_trailing_punct_hint as string)
        ).toBeInTheDocument();
      });
    });

    it('PUNCT-214-UI-002: defaults to unchecked when config.punctuation is absent', async () => {
      // baseConfig 无 punctuation 字段（= 存量用户旧 config）→ 新开关必须默认关。
      render(<VoicePage config={baseConfig} updateConfig={vi.fn()} />);
      await waitFor(() => {
        const boxes = screen.getAllByRole('checkbox') as HTMLInputElement[];
        expect(boxes.length).toBe(2);
        expect(boxes[1].checked).toBe(false);
        // 对照：自动补全标点默认开（缺字段回落 true），两条默认值互不牵连
        expect(boxes[0].checked).toBe(true);
      });
    });

    it('PUNCT-214-UI-003: reflects config.punctuation.strip_trailing = true', async () => {
      const config = {
        ...baseConfig,
        punctuation: { enabled: true, strip_trailing: true },
      };
      render(<VoicePage config={config} updateConfig={vi.fn()} />);
      await waitFor(() => {
        const boxes = screen.getAllByRole('checkbox') as HTMLInputElement[];
        expect(boxes[1].checked).toBe(true);
      });
    });

    it('PUNCT-214-UI-004: click writes back punctuation.strip_trailing（字段名契约）', async () => {
      const updateConfig = vi.fn();
      render(<VoicePage config={baseConfig} updateConfig={updateConfig} />);
      await waitFor(() => {
        const boxes = screen.getAllByRole('checkbox') as HTMLInputElement[];
        fireEvent.click(boxes[1]);
      });
      expect(updateConfig).toHaveBeenCalledWith(
        expect.objectContaining({
          punctuation: expect.objectContaining({ strip_trailing: true }),
        })
      );
    });

    it('PUNCT-214-UI-005: click does not disturb punctuation.enabled（不串字段）', async () => {
      const updateConfig = vi.fn();
      const config = {
        ...baseConfig,
        punctuation: { enabled: false, strip_trailing: false },
      };
      render(<VoicePage config={config} updateConfig={updateConfig} />);
      await waitFor(() => {
        const boxes = screen.getAllByRole('checkbox') as HTMLInputElement[];
        fireEvent.click(boxes[1]);
      });
      const last = updateConfig.mock.calls[updateConfig.mock.calls.length - 1][0];
      // 展开必须保留原 enabled=false（不得被默认 true 覆盖）
      expect(last.punctuation.enabled).toBe(false);
      expect(last.punctuation.strip_trailing).toBe(true);
    });

    it('PUNCT-214-UI-006: i18n key present in all three locales（防 HANT 漏 key）', async () => {
      // I18N-HANT-GAP-001 教训：三份 locale 必须同时补 key。
      const zhHant = (await import('../i18n/zh-Hant')).zhHant;
      const en = (await import('../i18n/en')).en;
      const locales: Array<[string, any]> = [
        ['zh-Hans', zhHans],
        ['zh-Hant', zhHant],
        ['en', en],
      ];
      for (const [name, dict] of locales) {
        expect(
          dict.voice_strip_trailing_punct,
          `${name} 缺 voice_strip_trailing_punct`
        ).toBeTruthy();
        expect(
          dict.voice_strip_trailing_punct_hint,
          `${name} 缺 voice_strip_trailing_punct_hint`
        ).toBeTruthy();
      }
    });
  });

  describe('GUARD-225-226 · UI-ASRKEY-225 + UI-LLMHINT-226', () => {
    // 在线 ASR 的 Key 输入框只在 asr_model ∈ {qwen_audio_online, fun_asr_realtime} 时渲染。
    const onlineConfig = {
      ...baseConfig,
      audio: { ...baseConfig.audio, asr_model: 'fun_asr_realtime' },
    };

    const loadLocales = async (): Promise<Array<[string, Record<string, string>]>> => {
      // 三份 locale 同时载入，专防 I18N-HANT-GAP-001（繁中漏 key 不报错）
      const zhHant = (await import('../i18n/zh-Hant')).zhHant as unknown as Record<
        string,
        string
      >;
      const en = (await import('../i18n/en')).en as unknown as Record<string, string>;
      return [
        ['zh-Hans', zhHans as unknown as Record<string, string>],
        ['zh-Hant', zhHant],
        ['en', en],
      ];
    };

    // ────────────────────────────────────────────────────────────
    // 225 · 🔴 一票否决核心不变量（Gavin）：显示灰显提示文字 == 输入框为空、未输入 key。
    //      ⇒ 提示文案只允许存在于 placeholder 属性，绝不允许进入 value / state / 落盘。
    // ────────────────────────────────────────────────────────────
    it('UI-ASRKEY-225-001: 空值时提示文案只在 placeholder，value 为空且挂载后零写回', async () => {
      const updateConfig = vi.fn();
      render(<VoicePage config={onlineConfig} updateConfig={updateConfig} />);
      const input = (await screen.findByPlaceholderText(
        zhHans.voice_asr_online_api_key_placeholder as string
      )) as HTMLInputElement;

      // ① value 必须为空
      expect(input.value).toBe('');
      // ② 提示文案出现在 placeholder 属性上（证明它在 placeholder 而非 value）
      expect(input.getAttribute('placeholder')).toBe(
        zhHans.voice_asr_online_api_key_placeholder
      );
      // ③ 提示文案绝不出现在 value 里（一票否决判据的直接形式）
      expect(input.value).not.toContain(
        zhHans.voice_asr_online_api_key_placeholder as string
      );
      // ④ 空值挂载不触发任何写回 ⇒ 提示文案不可能被写进 state/config.toml
      expect(updateConfig).not.toHaveBeenCalled();
    });

    it('UI-ASRKEY-225-002: 空值时测试连接按钮 disabled；输入后回调载荷不含提示文案', async () => {
      const updateConfig = vi.fn();
      render(<VoicePage config={onlineConfig} updateConfig={updateConfig} />);
      const input = (await screen.findByPlaceholderText(
        zhHans.voice_asr_online_api_key_placeholder as string
      )) as HTMLInputElement;

      const testBtn = screen.getByRole('button', {
        name: zhHans.voice_qwen3_test_connection as string,
      });
      expect(testBtn).toBeDisabled();

      fireEvent.change(input, { target: { value: 'sk-real-key' } });
      expect(updateConfig).toHaveBeenCalledWith(
        expect.objectContaining({
          audio: expect.objectContaining({ asr_online_api_key: 'sk-real-key' }),
        })
      );
      // 写回载荷里绝不能出现提示文案
      const payload = JSON.stringify(
        updateConfig.mock.calls[updateConfig.mock.calls.length - 1][0]
      );
      expect(payload).not.toContain(
        zhHans.voice_asr_online_api_key_placeholder as string
      );
    });

    it('UI-ASRKEY-225-003: 输入框类名 asr-key-input + password 类型，label 取 i18n 值', async () => {
      render(<VoicePage config={onlineConfig} updateConfig={vi.fn()} />);
      const input = (await screen.findByPlaceholderText(
        zhHans.voice_asr_online_api_key_placeholder as string
      )) as HTMLInputElement;

      expect(input.className).toContain('asr-key-input');
      expect(input.getAttribute('type')).toBe('password');
      expect(
        screen.getByText(zhHans.voice_asr_online_api_key as string)
      ).toBeInTheDocument();
    });

    // ────────────────────────────────────────────────────────────
    // 225 · 反硬编码（行为式）：切换语言后 placeholder/label 必须跟着变。
    //      若组件内写死了中文文案，英文/繁中渲染会露出中文 ⇒ 红。
    // ────────────────────────────────────────────────────────────
    it('UI-ASRKEY-225-004: 英文/繁中界面下 placeholder 与 label 随 locale 变化（行为式反硬编码）', async () => {
      const locales = await loadLocales();
      const dictOf = (name: string) =>
        locales.find(([n]) => n === name)![1] as Record<string, string>;
      // 🔴 ui_language 是枚举值（'English' / 'TraditionalChinese'），不是 locale 目录名
      const pairs: Array<[string, Record<string, string>]> = [
        ['English', dictOf('en')],
        ['TraditionalChinese', dictOf('zh-Hant')],
      ];
      for (const [uiLang, dict] of pairs) {
        cleanup();
        render(
          <VoicePage
            config={{ ...onlineConfig, ui_language: uiLang }}
            updateConfig={vi.fn()}
          />
        );
        const input = (await screen.findByPlaceholderText(
          dict.voice_asr_online_api_key_placeholder
        )) as HTMLInputElement;
        expect(input.getAttribute('placeholder')).toBe(
          dict.voice_asr_online_api_key_placeholder
        );
        expect(screen.getByText(dict.voice_asr_online_api_key)).toBeInTheDocument();
        // 硬编码中文时英文界面会露出中文 ⇒ 本条即红
        expect(input.getAttribute('placeholder')).not.toContain('阿里云百炼');
      }
      cleanup();
    });

    // ────────────────────────────────────────────────────────────
    // 225 · 反硬编码（源码文本）：Gavin 通则「UI 修改都必须走 i18n」。
    // ────────────────────────────────────────────────────────────
    it('UI-ASRKEY-225-005: Voice.tsx 走 i18n 绑定且不含提示文案/标签字面量', async () => {
      // @ts-ignore - Vite 运行时支持 `?raw`；tsconfig 无 vite/client types，故忽略类型
      const voiceSrc: string = (await import('./Voice.tsx?raw')).default;

      expect(voiceSrc).toContain('t.voice_asr_online_api_key_placeholder');
      // 提示文案不得硬编码（显式字面量与其中的地名片段都禁）
      expect(voiceSrc).not.toContain(
        zhHans.voice_asr_online_api_key_placeholder as string
      );
      expect(voiceSrc).not.toContain('阿里云百炼');
    });

    it('UI-ASRKEY-225-006: 三份 locale 均含 placeholder key 且非空（防 HANT 漏 key）', async () => {
      const locales = await loadLocales();
      for (const [name, dict] of locales) {
        expect(
          typeof dict.voice_asr_online_api_key_placeholder,
          `${name} 缺 voice_asr_online_api_key_placeholder`
        ).toBe('string');
        expect(
          (dict.voice_asr_online_api_key_placeholder || '').length,
          `${name} 的 placeholder 为空串`
        ).toBeGreaterThan(0);
      }
      // Hans 改值核对（225 的标签改动）
      expect(zhHans.voice_asr_online_api_key).toContain('阿里云百炼');
    });

    // ────────────────────────────────────────────────────────────
    // 226 · llm_api_config 三份改值（i18n-only，组件零改动）
    // ────────────────────────────────────────────────────────────
    it('UI-LLMHINT-226-001: 三份 llm_api_config 均已改值（含建议模型段）', async () => {
      const locales = await loadLocales();
      for (const [name, dict] of locales) {
        expect(
          dict.llm_api_config,
          `${name} 的 llm_api_config 未含建议模型 deepseek-flash`
        ).toContain('deepseek-flash');
      }
    });

    it('UI-LLMHINT-226-002: Llm.tsx 零改动契约 —— 仍读 t.llm_api_config 且不硬编码新文案', async () => {
      // @ts-ignore - Vite 运行时支持 `?raw`；tsconfig 无 vite/client types，故忽略类型
      const llmSrc: string = (await import('./Llm.tsx?raw')).default;

      // 组件仍通过 i18n 取标签（226 是纯数据改动）
      expect(llmSrc).toContain('t.llm_api_config');
      // 新文案不得硬编码进组件
      expect(llmSrc).not.toContain('deepseek-flash');
      expect(llmSrc).not.toContain('参数格式');
    });
  });
});
