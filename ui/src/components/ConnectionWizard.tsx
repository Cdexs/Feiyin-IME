// FLASH-P3-C：首次连接向导（任务书 FlashASR/collab/p3c-spec.md §2）。
// 主程序启动时判「连接不可用」⇒ 以 `--wizard` 拉起设置界面直接打开；语音页顶部提示卡也能打开。
// 三选一：本机 flashasr / 连另一台电脑的 flashasr（连接码）/ 直连阿里云（API Key）。
// 连接信息：识别服务配对存 asr-remote.json（命令见 src-tauri/src/asr_server.rs），档位与 Key 走现有保存配置流程
//（save_config → 主程序配置监视热重载），不新增配置项。「稍后设置」只关窗：下次启动仍按现状判、仍会弹。
import React, { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { errorText, RemoteView } from './FeiyinServerPairing';

interface Props {
  t: Record<string, string>;
  config: any;
  updateConfig: (cfg: any) => void;
  /** 关闭向导（`done` = 已设好连接）。 */
  onClose: (done: boolean) => void;
}

type Busy = '' | 'local' | 'remote' | 'aliyun';

const ConnectionWizard: React.FC<Props> = ({ t, config, updateConfig, onClose }) => {
  const [localAvailable, setLocalAvailable] = useState(false);
  const [code, setCode] = useState('');
  const [apiKey, setApiKey] = useState('');
  const [busy, setBusy] = useState<Busy>('');
  const [err, setErr] = useState<{ which: Busy; text: string } | null>(null);

  useEffect(() => {
    invoke<boolean>('asr_local_detect').then((b) => setLocalAvailable(b === true)).catch(() => {});
  }, []);

  const run = async (which: Busy, fn: () => Promise<void>, failText: (code: string) => string) => {
    setBusy(which);
    setErr(null);
    try {
      await fn();
    } catch (e: any) {
      setErr({ which, text: failText(typeof e === 'string' ? e : 'io') });
    } finally {
      setBusy('');
    }
  };

  /** 选定「连 flashasr」档位（配对已由命令落盘，录音时 load_for_use 取用）。
   *  Gavin 10-30：本机 flashasr 只在用户点「使用」确认后才写配对（asr_local_use），检测到只提示、不自动连。 */
  const selectServer = () => updateConfig({
    ...config,
    audio: { ...config.audio, asr_model: 'feiyin_server' },
  });

  // 1. 本机 flashasr：写配对（本机）+ 档位，再顺手测一次连接；通过即完成，不通显示原因（档位已写，本机可用）。
  const chooseLocal = () => run('local', async () => {
    await invoke<RemoteView>('asr_local_use');
    selectServer();
    await invoke('asr_remote_test');
    onClose(true);
  }, (c) => errorText(t, c));

  // 2. 另一台电脑：连接码配对 → 测试连接 → 通过才切档位。
  const chooseRemote = () => run('remote', async () => {
    await invoke<RemoteView>('asr_remote_pair_code', { code });
    await invoke('asr_remote_test');
    selectServer();
    onClose(true);
  }, (c) => errorText(t, c));

  // 3. 直连阿里：测 Key → 通过才写档位 + Key（同语音页切档：一次写全，asr_online_model 同步为 fun-asr 默认串）。
  //    测试命令返回的错误是内部原文，不上屏，统一显示 i18n 文案。
  const chooseAliyun = () => run('aliyun', async () => {
    await invoke<string>('test_qwen3_asr_connection', { apiKey: apiKey.trim() });
    updateConfig({
      ...config,
      audio: {
        ...config.audio,
        asr_model: 'fun_asr_realtime',
        asr_online_model: 'fun-asr-realtime',
        asr_online_api_key: apiKey.trim(),
      },
    });
    onClose(true);
  }, () => t.wizard_aliyun_failed);

  const errFor = (which: Busy) => err && err.which === which && (
    <p className="form-hint qwen3-test-failed" style={{ marginTop: '8px' }} role="alert">{err.text}</p>
  );

  return (
    <div className="modal-overlay">
      <div className="modal-dialog" role="dialog" aria-modal="true" aria-label={t.wizard_title}>
        <div className="modal-header">
          <svg className="modal-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <path d="M12 1a3 3 0 00-3 3v8a3 3 0 006 0V4a3 3 0 00-3-3z" />
            <path d="M19 10v2a7 7 0 01-14 0v-2" />
          </svg>
          <span className="modal-title">{t.wizard_title}</span>
          <button className="modal-close" onClick={() => onClose(false)}>×</button>
        </div>
        <div className="modal-body">
          <p className="form-hint" style={{ marginBottom: '12px' }}>{t.wizard_intro}</p>

          <div className="card">
            <div className="section-subtitle">{t.wizard_local_title}</div>
            <p className="form-hint">{localAvailable ? t.wizard_local_found : t.wizard_local_missing}</p>
            <button
              type="button"
              className="btn btn-primary btn-sm"
              style={{ marginTop: '8px' }}
              onClick={chooseLocal}
              disabled={!localAvailable || busy !== ''}
            >
              {busy === 'local' ? t.wizard_connecting : t.wizard_use}
            </button>
            {errFor('local')}
          </div>

          <div className="card">
            <div className="section-subtitle">{t.wizard_remote_title}</div>
            <p className="form-hint">{t.wizard_remote_hint}</p>
            <textarea
              className="input"
              rows={3}
              style={{ width: '100%', resize: 'vertical', marginTop: '8px' }}
              value={code}
              placeholder={t.feiyin_code_placeholder}
              aria-label={t.wizard_remote_title}
              onChange={(e) => setCode(e.target.value)}
            />
            <button
              type="button"
              className="btn btn-primary btn-sm"
              style={{ marginTop: '8px' }}
              onClick={chooseRemote}
              disabled={!code.trim() || busy !== ''}
            >
              {busy === 'remote' ? t.wizard_connecting : t.wizard_connect}
            </button>
            {errFor('remote')}
          </div>

          <div className="card">
            <div className="section-subtitle">{t.wizard_aliyun_title}</div>
            <p className="form-hint">{t.wizard_aliyun_hint}</p>
            <input
              type="password"
              className="input asr-key-input"
              style={{ marginTop: '8px' }}
              value={apiKey}
              placeholder={t.voice_asr_online_api_key_placeholder}
              aria-label={t.wizard_aliyun_title}
              onChange={(e) => setApiKey(e.target.value)}
            />
            <button
              type="button"
              className="btn btn-primary btn-sm"
              style={{ marginTop: '8px' }}
              onClick={chooseAliyun}
              disabled={!apiKey.trim() || busy !== ''}
            >
              {busy === 'aliyun' ? t.wizard_connecting : t.wizard_connect}
            </button>
            {errFor('aliyun')}
          </div>
        </div>
        <div className="modal-footer">
          <button type="button" className="btn btn-secondary" onClick={() => onClose(false)} disabled={busy !== ''}>
            {t.wizard_later}
          </button>
        </div>
      </div>
    </div>
  );
};

export default ConnectionWizard;
