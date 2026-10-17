// ASR-SERVER-491 S8：语音页「自家识别服务」档位的配对卡片（设计 §8.2 / §8.3 / §8.5）。
// 两种配对：粘贴连接码（自带证书指纹）/ 手动输入（先取回对方指纹让用户核对一次，同 SSH 首次连接）。
// 配对信息存 asr-remote.json（不进 config.toml），后端命令见 src-tauri/src/asr_server.rs。
import React, { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

type NetMode = 'auto' | 'lan' | 'wan';

interface RemoteView {
  paired: boolean;
  lan: string;
  wan: string;
  fingerprint: string;
  mode: NetMode;
  device_name: string;
  computer_name: string;
}

interface Probed {
  fingerprint: string;
  endpoint: string;
}

interface Props {
  t: Record<string, string>;
  /** 配对状态变化（语音页离开时据此决定是否回落档位）。 */
  onPairedChange: (paired: boolean) => void;
}

/** 指纹每 4 位一组，方便与对方窗口逐段核对。 */
export function formatFingerprint(fp: string): string {
  return (fp.match(/.{1,4}/g) || []).join(' ');
}

/** 后端错误码 → 文案键（不把内部错误原文上屏）。 */
function errorText(t: Record<string, string>, code: string): string {
  switch (code) {
    case 'bad_code': return t.feiyin_err_bad_code;
    case 'bad_address': return t.feiyin_err_bad_address;
    case 'missing_key': return t.feiyin_err_missing_key;
    case 'bad_fingerprint': return t.feiyin_err_bad_fingerprint;
    case 'unreachable': return t.feiyin_err_unreachable;
    case 'fingerprint_mismatch': return t.feiyin_err_fingerprint_mismatch;
    case 'InvalidKey':
    case 'KeyRevoked':
    case 'KeyExpired': return t.feiyin_err_key;
    case 'ServicePaused': return t.feiyin_err_paused;
    case 'not_paired': return t.feiyin_not_paired;
    default: return t.feiyin_err_io;
  }
}

const FeiyinServerPairing: React.FC<Props> = ({ t, onPairedChange }) => {
  const [view, setView] = useState<RemoteView | null>(null);
  const [repairing, setRepairing] = useState(false);
  const [tab, setTab] = useState<'code' | 'manual'>('code');
  const [code, setCode] = useState('');
  const [lan, setLan] = useState('');
  const [wan, setWan] = useState('');
  const [key, setKey] = useState('');
  const [probed, setProbed] = useState<Probed | null>(null);
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState('');
  const [deviceName, setDeviceName] = useState('');
  const [testMsg, setTestMsg] = useState('');

  const apply = (v: RemoteView) => {
    setView(v);
    setDeviceName(v.device_name);
    onPairedChange(v.paired);
  };

  useEffect(() => {
    invoke<RemoteView>('asr_remote_get').then(apply).catch(() => {});
  }, []);

  const run = async (fn: () => Promise<void>) => {
    setBusy(true);
    setErr('');
    try {
      await fn();
    } catch (e: any) {
      setErr(typeof e === 'string' ? e : 'io');
    } finally {
      setBusy(false);
    }
  };

  const finishPairing = (v: RemoteView) => {
    apply(v);
    setRepairing(false);
    setCode('');
    setKey('');
    setProbed(null);
    setTestMsg('');
  };

  const pairWithCode = () => run(async () => {
    finishPairing(await invoke<RemoteView>('asr_remote_pair_code', { code }));
  });

  const probe = () => run(async () => {
    setProbed(await invoke<Probed>('asr_remote_probe', { lan, wan, key }));
  });

  const confirmManual = () => run(async () => {
    finishPairing(await invoke<RemoteView>('asr_remote_pair_manual', {
      lan, wan, key, fingerprint: probed!.fingerprint,
    }));
  });

  const update = (mode: NetMode, name: string) => run(async () => {
    apply(await invoke<RemoteView>('asr_remote_update', { mode, deviceName: name }));
  });

  const test = () => run(async () => {
    setTestMsg('');
    const r = await invoke<{ net: string; ms: number }>('asr_remote_test');
    setTestMsg(t.feiyin_test_ok
      .replace('{net}', r.net === 'lan' ? t.asr_net_lan : t.asr_net_wan)
      .replace('{ms}', String(r.ms)));
  });

  if (!view) return null;
  const showForm = !view.paired || repairing;

  const netOptions: [NetMode, string][] = [
    ['auto', t.feiyin_net_auto],
    ['lan', t.feiyin_net_lan],
    ['wan', t.feiyin_net_wan],
  ];

  return (
    <div className="qwen3-section" style={{ marginTop: '16px' }}>
      {!showForm && (
        <>
          <div className="form-group">
            <span className="form-label">{t.feiyin_paired}</span>
            <p className="form-hint" style={{ wordBreak: 'break-all' }}>
              {view.lan && <>{t.feiyin_lan}{t.asr_colon}{view.lan}<br /></>}
              {view.wan && <>{t.feiyin_wan}{t.asr_colon}{view.wan}<br /></>}
              {t.asr_fingerprint}{t.asr_colon}<code>{formatFingerprint(view.fingerprint)}</code>
            </p>
          </div>

          <div className="form-group" style={{ marginTop: '12px' }}>
            <span className="form-label">{t.feiyin_net_mode}</span>
            <div className="radio-group" style={{ marginTop: '8px' }}>
              {netOptions.map(([m, label]) => (
                <label key={m} className={`radio-card ${view.mode === m ? 'active' : ''}`}>
                  <input
                    type="radio"
                    name="feiyin_net_mode"
                    checked={view.mode === m}
                    onChange={() => update(m, deviceName)}
                    className="radio-input"
                  />
                  <span className="custom-radio"></span>
                  <div className="radio-content">
                    <span className="radio-title">{label}</span>
                  </div>
                </label>
              ))}
            </div>
          </div>

          <div className="form-group" style={{ marginTop: '12px' }}>
            <span className="form-label">{t.feiyin_device_name}</span>
            <input
              className="input"
              value={deviceName}
              placeholder={view.computer_name}
              onChange={(e) => setDeviceName(e.target.value)}
              onBlur={() => { if (deviceName !== view.device_name) update(view.mode, deviceName); }}
            />
            <p className="form-hint">{t.feiyin_device_name_hint}</p>
          </div>

          <div className="form-group" style={{ marginTop: '12px' }}>
            <button type="button" className="btn btn-primary btn-sm" onClick={test} disabled={busy}>
              {busy ? t.feiyin_testing : t.feiyin_test}
            </button>
            <button
              type="button"
              className="btn btn-secondary btn-sm"
              style={{ marginLeft: '8px' }}
              onClick={() => { setRepairing(true); setErr(''); setTestMsg(''); }}
              disabled={busy}
            >
              {t.feiyin_repair}
            </button>
            {testMsg && (
              <span className="qwen3-test-result qwen3-test-success">{testMsg}</span>
            )}
          </div>
        </>
      )}

      {showForm && (
        <>
          {!view.paired && (
            <p className="form-hint" style={{ color: 'var(--brand-primary)', marginBottom: '8px' }}>
              {t.feiyin_not_paired}
            </p>
          )}
          <div className="radio-group">
            {(['code', 'manual'] as const).map((k) => (
              <label key={k} className={`radio-card ${tab === k ? 'active' : ''}`}>
                <input
                  type="radio"
                  name="feiyin_pair_tab"
                  checked={tab === k}
                  onChange={() => { setTab(k); setErr(''); setProbed(null); }}
                  className="radio-input"
                />
                <span className="custom-radio"></span>
                <div className="radio-content">
                  <span className="radio-title">{k === 'code' ? t.feiyin_tab_code : t.feiyin_tab_manual}</span>
                </div>
              </label>
            ))}
          </div>

          {tab === 'code' && (
            <div className="form-group" style={{ marginTop: '12px' }}>
              <textarea
                className="input"
                rows={3}
                style={{ width: '100%', resize: 'vertical' }}
                value={code}
                placeholder={t.feiyin_code_placeholder}
                onChange={(e) => setCode(e.target.value)}
              />
              <button
                type="button"
                className="btn btn-primary btn-sm"
                style={{ marginTop: '8px' }}
                onClick={pairWithCode}
                disabled={busy || !code.trim()}
              >
                {t.feiyin_pair}
              </button>
            </div>
          )}

          {tab === 'manual' && !probed && (
            <div style={{ marginTop: '12px' }}>
              <div className="form-group">
                <span className="form-label">{t.feiyin_lan}</span>
                <input className="input" value={lan} placeholder={t.feiyin_addr_placeholder}
                  onChange={(e) => setLan(e.target.value)} />
              </div>
              <div className="form-group">
                <span className="form-label">{t.feiyin_wan}</span>
                <input className="input" value={wan} placeholder={t.feiyin_addr_placeholder}
                  onChange={(e) => setWan(e.target.value)} />
              </div>
              <div className="form-group">
                <span className="form-label">{t.feiyin_key}</span>
                <input className="input" type="password" value={key}
                  onChange={(e) => setKey(e.target.value)} />
              </div>
              <button
                type="button"
                className="btn btn-primary btn-sm"
                onClick={probe}
                disabled={busy || !key.trim() || (!lan.trim() && !wan.trim())}
              >
                {busy ? t.feiyin_testing : t.feiyin_next}
              </button>
            </div>
          )}

          {tab === 'manual' && probed && (
            <div className="form-group" style={{ marginTop: '12px' }}>
              <span className="form-label">{t.feiyin_confirm_fp_title}</span>
              <p className="form-hint" style={{ wordBreak: 'break-all' }}>
                {probed.endpoint}<br />
                <code>{formatFingerprint(probed.fingerprint)}</code>
              </p>
              <p className="form-hint">{t.feiyin_confirm_fp_hint}</p>
              <button type="button" className="btn btn-primary btn-sm" onClick={confirmManual} disabled={busy}>
                {t.feiyin_confirm}
              </button>
              <button
                type="button"
                className="btn btn-secondary btn-sm"
                style={{ marginLeft: '8px' }}
                onClick={() => setProbed(null)}
                disabled={busy}
              >
                {t.feiyin_cancel}
              </button>
            </div>
          )}

          {view.paired && (
            <button
              type="button"
              className="btn btn-link"
              style={{ marginTop: '8px' }}
              onClick={() => { setRepairing(false); setErr(''); setProbed(null); }}
            >
              {t.feiyin_cancel}
            </button>
          )}
        </>
      )}

      {err && (
        <p className="form-hint qwen3-test-failed" style={{ marginTop: '8px' }}>
          {errorText(t, err)}
        </p>
      )}
    </div>
  );
};

export default FeiyinServerPairing;
