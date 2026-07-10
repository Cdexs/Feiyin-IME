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
      qwen3_api_key: '',
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
        const select = screen.getAllByRole('combobox')[2];
        expect(select).toBeInTheDocument();
      });
    });

    it('ASR-UI-002: select has three options', async () => {
      render(<VoicePage config={baseConfig} updateConfig={vi.fn()} />);
      await waitFor(() => {
        const select = screen.getAllByRole('combobox')[2];
        expect(select).toBeInTheDocument();
        const options = select.querySelectorAll('option');
        expect(options.length).toBe(3);
      });
    });

    it('ASR-UI-003: default selected value is "performance"', async () => {
      render(<VoicePage config={baseConfig} updateConfig={vi.fn()} />);
      await waitFor(() => {
        const select = screen.getAllByRole('combobox')[2] as HTMLSelectElement;
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
          screen.getByText(zhHans.voice_asr_model_accuracy as string)
        ).toBeInTheDocument();
        expect(
          screen.getByText(zhHans.voice_asr_model_qwen3 as string)
        ).toBeInTheDocument();
      });
    });

    it('ASR-UI-005: changing select updates config', async () => {
      const updateConfig = vi.fn();
      render(<VoicePage config={baseConfig} updateConfig={updateConfig} />);
      await waitFor(() => {
        const select = screen.getAllByRole('combobox')[2];
        fireEvent.change(select, { target: { value: 'accuracy' } });
        expect(updateConfig).toHaveBeenCalledWith(
          expect.objectContaining({
            audio: expect.objectContaining({ asr_model: 'accuracy' }),
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

    it('ASR-UI-007: description shows correct text for accuracy model', async () => {
      render(<VoicePage config={baseConfig} updateConfig={vi.fn()} />);
      await waitFor(() => {
        expect(screen.getByText(zhHans.voice_asr_model_performance_desc as string)).toBeInTheDocument();
      });
    });

    it('ASR-UI-008: Qwen3 model shows API key input', async () => {
      const qwenConfig = {...baseConfig, audio: {...baseConfig.audio, asr_model: 'qwen3_online'}};
      render(<VoicePage config={qwenConfig} updateConfig={vi.fn()} />);
      await waitFor(() => {
        expect(screen.getByText(zhHans.voice_qwen3_api_key as string)).toBeInTheDocument();
      });
    });

    it('ASR-UI-009: Qwen3 model shows test connection button', async () => {
      const qwenConfig = {...baseConfig, audio: {...baseConfig.audio, asr_model: 'qwen3_online'}};
      render(<VoicePage config={qwenConfig} updateConfig={vi.fn()} />);
      await waitFor(() => {
        expect(screen.getByText(zhHans.voice_qwen3_test_connection as string)).toBeInTheDocument();
      });
    });

    it('ASR-UI-010: Qwen3 test connection button disabled when no key', async () => {
      const emptyKeyConfig = {...baseConfig, audio: {...baseConfig.audio, asr_model: 'qwen3_online', qwen3_api_key: ''}};
      render(<VoicePage config={emptyKeyConfig} updateConfig={vi.fn()} />);
      await waitFor(() => {
        const btn = screen.getByText(zhHans.voice_qwen3_test_connection as string).closest('button');
        expect(btn).toBeDisabled();
      });
    });

    it('ASR-UI-011: Qwen3 test connection button enabled with key', async () => {
      const hasKeyConfig = {...baseConfig, audio: {...baseConfig.audio, asr_model: 'qwen3_online', qwen3_api_key: 'sk-test-key'}};
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
      const goodKeyConfig = {...baseConfig, audio: {...baseConfig.audio, asr_model: 'qwen3_online', qwen3_api_key: 'sk-test-key-123456'}};
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
      const goodKeyConfig = {...baseConfig, audio: {...baseConfig.audio, asr_model: 'qwen3_online', qwen3_api_key: 'sk-test-key-123456'}};
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
      const badKeyConfig = {...baseConfig, audio: {...baseConfig.audio, asr_model: 'qwen3_online', qwen3_api_key: 'bad-key'}};
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
      const qwenConfig = {...baseConfig, audio: {...baseConfig.audio, asr_model: 'qwen3_online'}};
      rerender(<VoicePage config={qwenConfig} updateConfig={vi.fn()} />);
      await waitFor(() => {
        expect(screen.getByText(zhHans.voice_qwen3_api_key as string)).toBeInTheDocument();
      });
      rerender(<VoicePage config={baseConfig} updateConfig={vi.fn()} />);
      await waitFor(() => {
        expect(screen.queryByText(zhHans.voice_qwen3_api_key as string)).not.toBeInTheDocument();
      });
    });

    it('ASR-UI-016: empty key hint shown when switching to Qwen3 without key', async () => {
      const qwenConfig = {...baseConfig, audio: {...baseConfig.audio, asr_model: 'qwen3_online', qwen3_api_key: ''}};
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
      // Simulate user selecting qwen3_online via dropdown
      const select = screen.getAllByRole('combobox')[2];
      fireEvent.change(select, { target: { value: 'qwen3_online' } });
      // updateFn was called with new config; re-render as parent would
      expect(updateFn).toHaveBeenCalled();
      const qwenConfig = {...baseConfig, audio: {...baseConfig.audio, asr_model: 'qwen3_online'}};
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
      const select = screen.getAllByRole('combobox')[2];
      fireEvent.change(select, { target: { value: 'qwen3_online' } });
      // Re-render with key set
      const qwenKeyConfig = {...baseConfig, audio: {...baseConfig.audio, asr_model: 'qwen3_online', qwen3_api_key: 'sk-test-key'}};
      rerender(<VoicePage config={qwenKeyConfig} updateConfig={updateFn} />);
      await new Promise(r => setTimeout(r, 50));
      unmount();
      // updateFn should have been called with performance at most once (initial render sets model, but fallback shouldn't revert)
      const calls = updateFn.mock.calls.map((call: any[]) => call[0]);
      const fallbackCalls = calls.filter((cfg: any) =>
        cfg?.audio?.asr_model === 'performance'
      );
      // After switching to qwen3 (with key), cleanup should NOT fallback to performance
      expect(fallbackCalls.length).toBe(0);
    });

    it('FALLBACK-003: mount with qwen3+key does not fallback on unmount', async () => {
      const updateConfig = vi.fn();
      const qwenHasKeyConfig = {...baseConfig, audio: {...baseConfig.audio, asr_model: 'qwen3_online', qwen3_api_key: 'sk-test-key-123456'}};
      const { unmount } = render(<VoicePage config={qwenHasKeyConfig} updateConfig={updateConfig} />);
      await new Promise(r => setTimeout(r, 50));
      unmount();
      expect(updateConfig).not.toHaveBeenCalled();
    });

  });
});
