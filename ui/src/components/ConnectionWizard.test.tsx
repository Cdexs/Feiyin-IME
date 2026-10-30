// FLASH-P3-C：首次连接向导三条路径的按钮可用性与结果（任务书 FlashASR/collab/p3c-spec.md §2）。
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';

const mockInvoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: any[]) => mockInvoke(...args),
}));

import ConnectionWizard from './ConnectionWizard';
import { zhHans } from '../i18n/zh-Hans';

const t = zhHans as unknown as Record<string, string>;
const config = {
  ui_language: 'Chinese',
  audio: { asr_model: 'feiyin_server', asr_online_api_key: '', asr_online_model: 'x' },
};
const VIEW = { paired: true, lan: '127.0.0.1:7860', wan: '', fingerprint: 'a', mode: 'lan', device_name: '', computer_name: 'PC', local: true };

function setup(handlers: Record<string, (args?: any) => any>) {
  mockInvoke.mockImplementation(async (cmd: string, args?: any) => {
    const h = handlers[cmd];
    return h ? h(args) : null;
  });
  const updateConfig = vi.fn();
  const onClose = vi.fn();
  render(<ConnectionWizard t={t} config={config} updateConfig={updateConfig} onClose={onClose} />);
  return { updateConfig, onClose };
}

const useBtn = () => screen.getByRole('button', { name: t.wizard_use });
const connectBtns = () => screen.getAllByRole('button', { name: t.wizard_connect });

describe('ConnectionWizard（FLASH-P3-C）', () => {
  beforeEach(() => mockInvoke.mockReset());

  it('P3C-WIZ-1: 本机没装 flashasr ⇒「使用」灰掉并提示；连接码 / Key 为空时「连接」灰掉', async () => {
    setup({ asr_local_detect: () => false });
    expect(await screen.findByText(t.wizard_local_missing)).toBeInTheDocument();
    expect(useBtn()).toBeDisabled();
    const [remote, aliyun] = connectBtns();
    expect(remote).toBeDisabled();
    expect(aliyun).toBeDisabled();
    fireEvent.change(screen.getByLabelText(t.wizard_remote_title), { target: { value: 'flash-asr://pair?v=1' } });
    expect(connectBtns()[0]).toBeEnabled();
    fireEvent.change(screen.getByLabelText(t.wizard_aliyun_title), { target: { value: 'sk-1' } });
    expect(connectBtns()[1]).toBeEnabled();
  });

  it('P3C-WIZ-2: 本机有 flashasr ⇒「使用」可点：写本机配对 + 档位 feiyin_server，测试通过后关闭', async () => {
    const { updateConfig, onClose } = setup({
      asr_local_detect: () => true,
      asr_local_use: () => VIEW,
      asr_remote_test: () => ({ net: 'lan', ms: 3 }),
    });
    expect(await screen.findByText(t.wizard_local_found)).toBeInTheDocument();
    await waitFor(() => expect(useBtn()).toBeEnabled());
    // Gavin 10-30：检测到本机只提示，不自动连 —— 用户点「使用」之前不写本机配对、不改档位。
    expect(mockInvoke).not.toHaveBeenCalledWith('asr_local_use');
    expect(updateConfig).not.toHaveBeenCalled();
    fireEvent.click(useBtn());
    await waitFor(() => expect(onClose).toHaveBeenCalledWith(true));
    expect(mockInvoke).toHaveBeenCalledWith('asr_local_use');
    expect(updateConfig).toHaveBeenCalledWith(
      expect.objectContaining({ audio: expect.objectContaining({ asr_model: 'feiyin_server' }) })
    );
  });

  it('P3C-WIZ-3: 连接码配对 → 测试连接 → 写档位；配对失败显示 i18n 错误、不写档位不关闭', async () => {
    let fail = true;
    const { updateConfig, onClose } = setup({
      asr_local_detect: () => false,
      asr_remote_pair_code: () => { if (fail) throw 'bad_code'; return { ...VIEW, local: false }; },
      asr_remote_test: () => ({ net: 'lan', ms: 3 }),
    });
    fireEvent.change(screen.getByLabelText(t.wizard_remote_title), { target: { value: 'xx' } });
    fireEvent.click(connectBtns()[0]);
    expect(await screen.findByText(t.feiyin_err_bad_code)).toBeInTheDocument();
    expect(updateConfig).not.toHaveBeenCalled();
    expect(onClose).not.toHaveBeenCalled();

    fail = false;
    fireEvent.click(connectBtns()[0]);
    await waitFor(() => expect(onClose).toHaveBeenCalledWith(true));
    expect(mockInvoke).toHaveBeenCalledWith('asr_remote_pair_code', { code: 'xx' });
    expect(mockInvoke).toHaveBeenCalledWith('asr_remote_test');
    expect(updateConfig).toHaveBeenCalledWith(
      expect.objectContaining({ audio: expect.objectContaining({ asr_model: 'feiyin_server' }) })
    );
  });

  it('P3C-WIZ-4: 直连阿里：测试失败只显示 i18n 文案（不上屏原文）；通过后写档位 + Key', async () => {
    let ok = false;
    const { updateConfig, onClose } = setup({
      asr_local_detect: () => false,
      test_qwen3_asr_connection: () => { if (!ok) throw 'API key 无效或鉴权失败：服务端拒绝连接（HTTP 401/403）'; return 'OK'; },
    });
    fireEvent.change(screen.getByLabelText(t.wizard_aliyun_title), { target: { value: ' sk-9 ' } });
    fireEvent.click(connectBtns()[1]);
    expect(await screen.findByText(t.wizard_aliyun_failed)).toBeInTheDocument();
    expect(screen.queryByText(/HTTP 401/)).not.toBeInTheDocument();
    expect(updateConfig).not.toHaveBeenCalled();

    ok = true;
    fireEvent.click(connectBtns()[1]);
    await waitFor(() => expect(onClose).toHaveBeenCalledWith(true));
    expect(mockInvoke).toHaveBeenCalledWith('test_qwen3_asr_connection', { apiKey: 'sk-9' });
    expect(updateConfig).toHaveBeenCalledWith(
      expect.objectContaining({
        audio: expect.objectContaining({
          asr_model: 'fun_asr_realtime',
          asr_online_model: 'fun-asr-realtime',
          asr_online_api_key: 'sk-9',
        }),
      })
    );
  });

  it('P3C-WIZ-5:「稍后设置」只关闭，不写配置', async () => {
    const { updateConfig, onClose } = setup({ asr_local_detect: () => true });
    await screen.findByText(t.wizard_local_found);
    fireEvent.click(screen.getByRole('button', { name: t.wizard_later }));
    expect(onClose).toHaveBeenCalledWith(false);
    expect(updateConfig).not.toHaveBeenCalled();
  });
});
