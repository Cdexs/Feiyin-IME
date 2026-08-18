import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';

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
        expect(checkboxes.length).toBe(1);
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

    it('ASR-UI-002: select has three options (performance / qwen / fun-asr)', async () => {
      render(<VoicePage config={baseConfig} updateConfig={vi.fn()} />);
      await waitFor(() => {
        const select = screen.getAllByRole('combobox')[1];
        expect(select).toBeInTheDocument();
        const options = select.querySelectorAll('option');
        expect(options.length).toBe(3);
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
        expect(
          screen.getByText(zhHans.voice_asr_model_qwen3 as string)
        ).toBeInTheDocument();
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
        fireEvent.change(select, { target: { value: 'qwen_audio_online' } });
        expect(updateConfig).toHaveBeenCalledWith(
          expect.objectContaining({
            audio: expect.objectContaining({ asr_model: 'qwen_audio_online' }),
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

    it('ASR-UI-008: Qwen3 model shows API key input', async () => {
      const qwenConfig = {...baseConfig, audio: {...baseConfig.audio, asr_model: 'qwen_audio_online'}};
      render(<VoicePage config={qwenConfig} updateConfig={vi.fn()} />);
      await waitFor(() => {
        expect(screen.getByText(zhHans.voice_asr_online_api_key as string)).toBeInTheDocument();
      });
    });

    it('ASR-UI-009: Qwen3 model shows test connection button', async () => {
      const qwenConfig = {...baseConfig, audio: {...baseConfig.audio, asr_model: 'qwen_audio_online'}};
      render(<VoicePage config={qwenConfig} updateConfig={vi.fn()} />);
      await waitFor(() => {
        expect(screen.getByText(zhHans.voice_qwen3_test_connection as string)).toBeInTheDocument();
      });
    });

    it('ASR-UI-010: Qwen3 test connection button disabled when no key', async () => {
      const emptyKeyConfig = {...baseConfig, audio: {...baseConfig.audio, asr_model: 'qwen_audio_online', asr_online_api_key: ''}};
      render(<VoicePage config={emptyKeyConfig} updateConfig={vi.fn()} />);
      await waitFor(() => {
        const btn = screen.getByText(zhHans.voice_qwen3_test_connection as string).closest('button');
        expect(btn).toBeDisabled();
      });
    });

    it('ASR-UI-011: Qwen3 test connection button enabled with key', async () => {
      const hasKeyConfig = {...baseConfig, audio: {...baseConfig.audio, asr_model: 'qwen_audio_online', asr_online_api_key: 'sk-test-key'}};
      render(<VoicePage config={hasKeyConfig} updateConfig={vi.fn()} />);
      await waitFor(() => {
        const btn = screen.getByText(zhHans.voice_qwen3_test_connection as string).closest('button');
        expect(btn).not.toBeDisabled();
      });
    });

    it('ASR-UI-012: clicking test connection shows testing state', async () => {
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

    it('ASR-UI-013: successful connection shows success message', async () => {
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

    it('ASR-UI-014: failed connection shows failure message', async () => {
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

    it('ASR-UI-015: switching models hides Qwen3 section', async () => {
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

    it('ASR-UI-016: empty key hint shown when switching to Qwen3 without key', async () => {
      const qwenConfig = {...baseConfig, audio: {...baseConfig.audio, asr_model: 'qwen_audio_online', asr_online_api_key: ''}};
      render(<VoicePage config={qwenConfig} updateConfig={vi.fn()} />);
      await waitFor(() => {
        expect(screen.getByText(zhHans.voice_qwen3_empty_key_hint as string)).toBeInTheDocument();
      });
    });
    // R2 fallback tests — use wrapper to simulate real parent state management
    it('FALLBACK-001: switch to qwen3(no key) then unmount falls back to performance', async () => {
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

    it('FALLBACK-002: switch to qwen3(with key) then unmount does NOT fallback', async () => {
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
        expect(values).toContain('qwen_audio_online');
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
});
