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
      asr_model: 'feiyin_server',
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

    // FLASH-P3-A（DEC-114）：模型下拉只剩「连识别服务」「直连阿里 FunASR」两项（本地档 / qwen 全删）。
    it('ASR-UI-002: select options are feiyin-server / fun-asr only', async () => {
      render(<VoicePage config={baseConfig} updateConfig={vi.fn()} />);
      await waitFor(() => {
        const select = screen.getAllByRole('combobox')[1];
        expect(select).toBeInTheDocument();
        const values = Array.from(select.querySelectorAll('option')).map(o => o.value);
        expect(values).toEqual(['feiyin_server', 'fun_asr_realtime']);
      });
    });

    // FLASH-P3-A（替换 ASR-UI-HIDE-FAST-503）：两个合法值各自选中时选项集合不变，旧档位不再以任何形式出现。
    it('P3A-UI-001: both valid models select themselves; legacy options never rendered', async () => {
      for (const model of ['fun_asr_realtime', 'feiyin_server']) {
        const cfg = {
          ...baseConfig,
          audio: { ...baseConfig.audio, asr_model: model },
        };
        const { unmount } = render(<VoicePage config={cfg} updateConfig={vi.fn()} />);
        await waitFor(() => {
          const select = screen.getAllByRole('combobox')[1] as HTMLSelectElement;
          const values = Array.from(select.querySelectorAll('option')).map(o => o.value);
          for (const gone of ['performance', 'accuracy', 'local_realtime', 'qwen_audio_online']) {
            expect(values).not.toContain(gone);
          }
          expect(select.value).toBe(model);
        });
        unmount();
      }
    });

    it('ASR-UI-003: default selected value is "feiyin_server"', async () => {
      render(<VoicePage config={baseConfig} updateConfig={vi.fn()} />);
      await waitFor(() => {
        const select = screen.getAllByRole('combobox')[1] as HTMLSelectElement;
        expect(select.value).toBe('feiyin_server');
      });
    });

    it('ASR-UI-004: options display correct i18n text', async () => {
      render(<VoicePage config={baseConfig} updateConfig={vi.fn()} />);
      await waitFor(() => {
        expect(
          screen.getByText(zhHans.voice_asr_model_feiyin as string)
        ).toBeInTheDocument();
        expect(
          screen.getByText(zhHans.voice_asr_model_fun_asr as string)
        ).toBeInTheDocument();
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
        const desc = screen.getByText(zhHans.voice_asr_model_feiyin_desc as string);
        expect(desc).toBeInTheDocument();
        expect(desc.className).toContain('asr-model-desc');
      });
    });

    it('ASR-UI-007: description shows correct text for feiyin_server model', async () => {
      render(<VoicePage config={baseConfig} updateConfig={vi.fn()} />);
      await waitFor(() => {
        expect(screen.getByText(zhHans.voice_asr_model_feiyin_desc as string)).toBeInTheDocument();
      });
    });

    // FLASH-P3-A：原 ASR-UI-008~016 / FALLBACK-001~003（qwen_audio_online 专属区块，已 skip）随该档删除；
    // 换成「新装默认连识别服务未配对，离开本页不白写配置」。
    it('FALLBACK-003: mount with feiyin_server (unpaired) does not write config on unmount', async () => {
      const updateConfig = vi.fn();
      const { unmount } = render(<VoicePage config={baseConfig} updateConfig={updateConfig} />);
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

    it('FUNFALLBACK-001: switch to fun_asr(no key) then unmount falls back to feiyin_server', async () => {
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
      const fallbackCalls = calls.filter((cfg: any) => cfg?.audio?.asr_model === 'feiyin_server');
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
    // 在线 ASR 的 Key 输入框只在 asr_model = fun_asr_realtime 时渲染（FLASH-P3-A 起 qwen 档已删）。
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

describe('VoicePage - 中文输出快捷方式（SC-SCRIPT-489）', () => {
  const config = {
    ui_language: 'Chinese' as const,
    hotkey: { mode: 'Toggle', vk_code: 0x78, modifiers: 0 },
    translation: { enabled: false, vk_code: 0 },
    audio: { device: 'default', asr_model: 'feiyin_server', chinese_script: 'Simplified', asr_online_api_key: '' },
    llm: { enabled: false, api_url: '', api_key: '', model: '' },
    punctuation: { enabled: true, strip_trailing: false },
  };

  beforeEach(() => {
    mockInvoke.mockReset();
    mockInvoke.mockImplementation(async () => null);
  });

  it('SC-UI-055: 「中文输出」后显示快捷方式 右Ctrl+3；点链接只开对话框，不改简繁', async () => {
    const updateConfig = vi.fn();
    render(<VoicePage config={config} updateConfig={updateConfig} />);
    const title = screen.getByText(zhHans.voice_chinese_output as string);
    const row = title.closest('.toggle-row') as HTMLElement;
    expect(row).toBeTruthy();
    expect(row.textContent).toContain('右Ctrl+3');
    const links = screen.getAllByRole('button', { name: zhHans.shortcut_link as string });
    expect(links.length).toBe(2); // 句尾标点 + 中文输出
    fireEvent.click(row.querySelector('.shortcut-link') as HTMLElement);
    expect(screen.getByRole('dialog')).toBeInTheDocument();
    expect(updateConfig).not.toHaveBeenCalled();
    expect(screen.getByRole('radio', { name: zhHans.voice_simplified as string })).toBeChecked();
  });
});

// FLASH-P3-C：连接不可用 ⇒ 页顶向导入口；本机 flashasr 只提示「改用本机」，用户点了才连（Gavin 10-30）。
describe('VoicePage - 首次连接向导入口（FLASH-P3-C）', () => {
  const config = {
    ui_language: 'Chinese' as const,
    hotkey: { mode: 'Toggle', vk_code: 0x78, modifiers: 0 },
    translation: { enabled: false, vk_code: 0 },
    audio: { device: 'default', asr_model: 'feiyin_server', chinese_script: 'Simplified', asr_online_api_key: '' },
    llm: { enabled: false, api_url: '', api_key: '', model: '' },
    punctuation: { enabled: true, strip_trailing: false },
  };
  const unpaired = { paired: false, lan: '', wan: '', fingerprint: '', mode: 'auto', device_name: '', computer_name: 'PC', local: false };

  beforeEach(() => {
    mockInvoke.mockReset();
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'asr_connection_state') return { ready: false, local_available: true };
      if (cmd === 'asr_remote_get') return unpaired;
      if (cmd === 'asr_local_detect') return true;
      if (cmd === 'asr_local_use') return { ...unpaired, paired: true, lan: '127.0.0.1:7860', local: true };
      return null;
    });
  });

  it('P3C-VOICE-1: 连接不可用 ⇒ 显示提示卡，点「设置连接」打开向导', async () => {
    const onOpenWizard = vi.fn();
    render(<VoicePage config={config} updateConfig={vi.fn()} onOpenWizard={onOpenWizard} />);
    fireEvent.click(await screen.findByRole('button', { name: zhHans.voice_conn_setup as string }));
    expect(onOpenWizard).toHaveBeenCalled();
    expect(screen.getByText(zhHans.voice_conn_not_ready as string)).toBeInTheDocument();
  });

  it('P3C-VOICE-2: 本机有 flashasr 不自动连；点「改用本机」后显示已连接本机、提示卡消失', async () => {
    render(<VoicePage config={config} updateConfig={vi.fn()} onOpenWizard={vi.fn()} />);
    const btn = await screen.findByRole('button', { name: zhHans.feiyin_use_local as string });
    expect(mockInvoke).not.toHaveBeenCalledWith('asr_local_use');
    fireEvent.click(btn);
    expect(await screen.findByText(zhHans.feiyin_local_paired as string)).toBeInTheDocument();
    expect(mockInvoke).toHaveBeenCalledWith('asr_local_use');
    expect(screen.queryByText(zhHans.voice_conn_not_ready as string)).not.toBeInTheDocument();
  });

  it('P3C-VOICE-3: 连接可用 ⇒ 不显示提示卡', async () => {
    mockInvoke.mockImplementation(async (cmd: string) =>
      cmd === 'asr_connection_state' ? { ready: true, local_available: false } : null);
    render(<VoicePage config={config} updateConfig={vi.fn()} onOpenWizard={vi.fn()} />);
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('asr_connection_state', { asrModel: 'feiyin_server', apiKey: '' }));
    expect(screen.queryByText(zhHans.voice_conn_not_ready as string)).not.toBeInTheDocument();
  });
});
