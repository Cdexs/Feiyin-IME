import React, { useCallback, useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { getTranslations } from '../i18n';
import { formatMinutes } from './Mine';

// ASR-SERVER-491 S6（设计 §16）：「识别服务」页 —— 服务状态 / 终端概况 / 终端列表三块卡片。
// 启动 / 停止 = 总开关（DEC-031，写 asr-service.json，内核看护线程跟上；托盘同此）；
// 暂停 / 添加终端 / 撤销经网关本机控制通道；统计只读 usage-stats.sqlite。

export interface TerminalRow {
  id: string;
  device_name: string;
  device_id: string;
  bound: boolean;
  revoked: boolean;
  created_ts: number;
  last_login_ts: number;
  last_logout_ts: number;
  net: string;
  online: boolean;
  week_speech_ms: number;
  week_words: number;
  total_speech_ms: number;
  total_words: number;
}

export interface Overview {
  enabled: boolean;
  port: number;
  wan_host: string;
  wan_port: number;
  running: boolean;
  paused: boolean;
  lan: string;
  fingerprint: string;
  online_count: number;
  paired_count: number;
  terminals: TerminalRow[];
}

interface AddedTerminal {
  terminal_id: string;
  key: string;
  lan: string;
  wan: string;
  uri: string;
  qr_svg: string;
  fingerprint: string;
}

interface SessionRow {
  login_ts: number;
  logout_ts: number | null;
}

interface Props {
  config: any;
  updateConfig: (cfg: any) => void;
}

/** 刷新间隔：在线状态靠轮询（控制通道一问一答，开销可忽略）。 */
const POLL_MS = 3000;

/** 服务状态（界面口径）。 */
export type ServiceState = 'not_ready' | 'stopped' | 'starting' | 'running' | 'paused';

export const serviceState = (ov: Overview | null, ready: boolean): ServiceState => {
  if (!ready) return 'not_ready';
  if (!ov || !ov.enabled) return 'stopped';
  if (!ov.running) return 'starting';
  return ov.paused ? 'paused' : 'running';
};

const fmtTs = (ts: number | null | undefined, dash: string): string =>
  ts && ts > 0 ? new Date(ts * 1000).toLocaleString() : dash;

const fill = (s: string, vars: Record<string, string | number>): string =>
  Object.entries(vars).reduce((acc, [k, v]) => acc.split(`{${k}}`).join(String(v)), s);

const AsrServerPage: React.FC<Props> = ({ config }) => {
  const t = getTranslations(config.ui_language);
  // 识别服务复用本机「本地流式实时模型」（设计 §8.1 / §16.2「未就绪」）。
  const ready = config.audio?.asr_model === 'local_realtime';

  const [ov, setOv] = useState<Overview | null>(null);
  const [loadFailed, setLoadFailed] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const [wanHost, setWanHost] = useState('');
  const [wanPort, setWanPort] = useState('');
  const [wanDirty, setWanDirty] = useState(false);
  const [wanSaved, setWanSaved] = useState(false);
  const [added, setAdded] = useState<AddedTerminal | null>(null);
  const [copied, setCopied] = useState('');
  const [revoking, setRevoking] = useState<TerminalRow | null>(null);
  const [expanded, setExpanded] = useState<string | null>(null);
  const [sessions, setSessions] = useState<SessionRow[]>([]);

  const errText = (code: unknown): string => {
    switch (String(code)) {
      case 'not_running': return t.asr_err_not_running;
      case 'not_found': return t.asr_err_not_found;
      default: return t.asr_err_io;
    }
  };

  const refresh = useCallback(async () => {
    try {
      const data = await invoke<Overview>('asr_server_overview');
      setOv(data);
      setLoadFailed(false);
      if (!wanDirty) {
        setWanHost(data.wan_host);
        setWanPort(data.wan_port ? String(data.wan_port) : '');
      }
    } catch (e) {
      console.error('Failed to load ASR service overview:', e);
      setLoadFailed(true);
    }
  }, [wanDirty]);

  useEffect(() => {
    refresh();
    const timer = setInterval(refresh, POLL_MS);
    return () => clearInterval(timer);
  }, [refresh]);

  const run = async (cmd: string, args?: Record<string, unknown>) => {
    setBusy(true);
    setError('');
    try {
      await invoke(cmd, args);
      await refresh();
    } catch (e) {
      setError(errText(e));
    } finally {
      setBusy(false);
    }
  };

  const saveWan = async () => {
    const port = parseInt(wanPort, 10);
    await run('asr_server_save_wan', {
      wanHost: wanHost.trim(),
      wanPort: Number.isFinite(port) && port > 0 && port < 65536 ? port : 0,
    });
    setWanDirty(false);
    setWanSaved(true);
    setTimeout(() => setWanSaved(false), 2000);
  };

  const addTerminal = async () => {
    setBusy(true);
    setError('');
    try {
      setAdded(await invoke<AddedTerminal>('asr_server_add_terminal'));
    } catch (e) {
      setError(errText(e));
    } finally {
      setBusy(false);
    }
  };

  const copy = async (label: string, text: string) => {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(label);
      setTimeout(() => setCopied(''), 1500);
    } catch (e) {
      console.error('Copy failed:', e);
    }
  };

  const toggleDetail = async (row: TerminalRow) => {
    if (expanded === row.id) {
      setExpanded(null);
      return;
    }
    setExpanded(row.id);
    setSessions([]);
    try {
      setSessions(await invoke<SessionRow[]>('asr_server_sessions', { terminalId: row.id }));
    } catch (e) {
      console.error('Failed to load terminal sessions:', e);
    }
  };

  const state = serviceState(ov, ready);
  const stateLabel: Record<ServiceState, string> = {
    not_ready: t.asr_status_not_ready,
    stopped: t.asr_status_stopped,
    starting: t.asr_status_starting,
    running: t.asr_status_running,
    paused: t.asr_status_paused,
  };
  const stateColor: Record<ServiceState, string> = {
    not_ready: 'var(--system-text-tertiary)',
    stopped: 'var(--system-text-secondary)',
    starting: 'var(--status-warning)',
    running: 'var(--status-success)',
    paused: 'var(--status-warning)',
  };
  const hintStyle: React.CSSProperties = { fontSize: '13px', color: 'var(--system-text-tertiary)', margin: '12px 0 0', lineHeight: 1.6 };
  const lineStyle: React.CSSProperties = { fontSize: '15px', lineHeight: 1.8, margin: '0 0 8px' };
  const dash = t.asr_dash;
  const netLabel = (n: string) => (n === 'wan' ? t.asr_net_wan : n === 'lan' ? t.asr_net_lan : dash);

  return (
    <div className="settings-page">
      <h2 className="page-title">{t.asr_title}</h2>

      {/* ① 服务状态 */}
      <section className="settings-section">
        <h3 className="section-title">{t.asr_status_section}</h3>
        <div className="card" data-testid="asr-status-card">
          {loadFailed && <p style={{ ...lineStyle, color: 'var(--status-error)' }}>{t.asr_load_failed}</p>}
          <p style={lineStyle} data-testid="asr-state">
            {t.asr_status_label}
            <strong style={{ color: stateColor[state] }}>{stateLabel[state]}</strong>
          </p>
          {state === 'not_ready' && <p style={hintStyle}>{t.asr_not_ready_hint}</p>}
          {state === 'starting' && <p style={hintStyle}>{t.asr_starting_hint}</p>}

          <div className="form-row" style={{ marginTop: '12px' }}>
            <label className="form-label">{t.asr_lan_label}</label>
            <span data-testid="asr-lan">{ov?.lan || t.asr_lan_unknown}</span>
          </div>
          <div className="form-row">
            <label className="form-label">{t.asr_wan_label}</label>
            <input
              className="input input-long"
              type="text"
              value={wanHost}
              placeholder={t.asr_wan_host_placeholder}
              onChange={(e) => { setWanHost(e.target.value); setWanDirty(true); }}
            />
            <input
              className="input input-short"
              type="text"
              inputMode="numeric"
              value={wanPort}
              placeholder={ov ? String(ov.port) : t.asr_wan_port_placeholder}
              onChange={(e) => { setWanPort(e.target.value.replace(/[^0-9]/g, '')); setWanDirty(true); }}
            />
            <button className="btn btn-secondary btn-sm" disabled={busy || !wanDirty} onClick={saveWan}>
              {wanSaved ? t.asr_saved : t.asr_save}
            </button>
          </div>
          <p style={hintStyle}>{fill(t.asr_wan_hint, { port: ov?.port ?? '' })}</p>

          <div style={{ display: 'flex', gap: '8px', marginTop: '16px' }}>
            {state === 'stopped' || state === 'not_ready' ? (
              <button
                className="btn btn-primary btn-accent"
                data-testid="asr-start"
                disabled={busy || state === 'not_ready'}
                onClick={() => run('asr_server_set_enabled', { enabled: true })}
              >
                {t.asr_start}
              </button>
            ) : (
              <>
                {state === 'paused' ? (
                  <button className="btn btn-primary btn-accent" disabled={busy} onClick={() => run('asr_server_set_paused', { paused: false })}>
                    {t.asr_resume}
                  </button>
                ) : (
                  <button className="btn btn-secondary" data-testid="asr-pause" disabled={busy || state !== 'running'} onClick={() => run('asr_server_set_paused', { paused: true })}>
                    {t.asr_pause}
                  </button>
                )}
                <button className="btn btn-secondary" data-testid="asr-stop" disabled={busy} onClick={() => run('asr_server_set_enabled', { enabled: false })}>
                  {t.asr_stop}
                </button>
              </>
            )}
          </div>
          {(state === 'running' || state === 'paused') && <p style={hintStyle}>{t.asr_pause_hint}</p>}
          {error && <p style={{ ...hintStyle, color: 'var(--status-error)' }}>{error}</p>}
        </div>
      </section>

      {/* ② 终端概况 */}
      <section className="settings-section">
        <h3 className="section-title">{t.asr_terminals_section}</h3>
        <div className="card" style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
          <span style={{ fontSize: '15px' }} data-testid="asr-counts">
            {fill(t.asr_online_paired, { online: ov?.online_count ?? 0, paired: ov?.paired_count ?? 0 })}
          </span>
          <button className="btn btn-primary btn-accent" data-testid="asr-add" disabled={busy || state !== 'running'} onClick={addTerminal}>
            {t.asr_add_terminal}
          </button>
        </div>
      </section>

      {/* ③ 终端列表 */}
      <section className="settings-section">
        <h3 className="section-title">{t.asr_list_section}</h3>
        <div className="card" data-testid="asr-list">
          {(!ov || ov.terminals.length === 0) && <p style={{ ...lineStyle, color: 'var(--system-text-tertiary)' }}>{t.asr_list_empty}</p>}
          {ov?.terminals.map((row) => (
            <div key={row.id} data-testid={`asr-terminal-${row.id}`} style={{ padding: '10px 0', borderBottom: '1px solid rgba(0,0,0,0.06)' }}>
              <div style={{ display: 'flex', alignItems: 'center', gap: '12px', cursor: 'pointer' }} onClick={() => toggleDetail(row)}>
                <strong style={{ fontSize: '15px' }}>{row.device_name || dash}</strong>
                <span style={{ fontSize: '13px', color: row.revoked ? 'var(--status-error)' : row.online ? 'var(--status-success)' : 'var(--system-text-tertiary)' }}>
                  {row.revoked ? t.asr_revoked : row.online ? t.asr_online : t.asr_offline}
                </span>
                <span style={{ fontSize: '13px', color: 'var(--system-text-secondary)' }}>{netLabel(row.net)}</span>
                <span style={{ flex: 1 }} />
                {!row.revoked && (
                  <button className="btn btn-link" onClick={(e) => { e.stopPropagation(); setRevoking(row); }}>
                    {t.asr_revoke}
                  </button>
                )}
              </div>
              <div style={{ fontSize: '13px', color: 'var(--system-text-secondary)', lineHeight: 1.8 }}>
                {t.asr_device_id}{row.device_id || dash}
                {t.asr_sep}{t.asr_last_login}{fmtTs(row.last_login_ts, dash)}
                {t.asr_sep}{t.asr_last_logout}{fmtTs(row.last_logout_ts, dash)}
                {t.asr_sep}{t.asr_week}{t.asr_duration}{formatMinutes(row.week_speech_ms)}{t.asr_minutes}
                {t.asr_sep_inner}{t.asr_words}{row.week_words}
              </div>
              {expanded === row.id && (
                <div style={{ fontSize: '13px', lineHeight: 1.8, margin: '6px 0 0 12px' }}>
                  <div>
                    {t.asr_total}{t.asr_duration}{formatMinutes(row.total_speech_ms)}{t.asr_minutes}
                    {t.asr_sep_inner}{t.asr_words}{row.total_words}
                  </div>
                  <div style={{ color: 'var(--system-text-secondary)' }}>{t.asr_sessions}</div>
                  {sessions.length === 0 && <div style={{ color: 'var(--system-text-tertiary)' }}>{t.asr_sessions_empty}</div>}
                  <ul style={{ listStyle: 'disc', margin: 0, padding: '0 0 0 20px' }}>
                    {sessions.map((s, i) => (
                      <li key={i}>
                        {fmtTs(s.login_ts, dash)}{' — '}
                        {s.logout_ts
                          ? `${fmtTs(s.logout_ts, dash)}${t.asr_paren_open}${formatMinutes((s.logout_ts - s.login_ts) * 1000)}${t.asr_minutes}${t.asr_paren_close}`
                          : t.asr_session_open}
                      </li>
                    ))}
                  </ul>
                </div>
              )}
            </div>
          ))}
          <p style={hintStyle}>{t.asr_rules_note}</p>
        </div>
      </section>

      {/* 添加终端：二维码 + 连接码 + 手输信息（§8.2） */}
      {added && (
        <div className="modal-overlay">
          <div className="modal-dialog" style={{ maxWidth: '480px' }} role="dialog" data-testid="asr-add-dialog" onClick={(e) => e.stopPropagation()}>
            <div className="modal-header">
              <span className="modal-title">{t.asr_add_title}</span>
              <button className="modal-close" onClick={() => setAdded(null)}>×</button>
            </div>
            <div className="modal-body">
              {added.qr_svg && (
                <div style={{ display: 'flex', justifyContent: 'center' }} dangerouslySetInnerHTML={{ __html: added.qr_svg }} />
              )}
              <p style={{ ...hintStyle, textAlign: 'center', margin: '4px 0 12px' }}>{t.asr_add_qr_hint}</p>
              <div className="form-group">
                <label className="form-label">{t.asr_conn_code}</label>
                <div style={{ display: 'flex', gap: '8px' }}>
                  <input className="input" readOnly value={added.uri} onFocus={(e) => e.target.select()} />
                  <button className="btn btn-secondary btn-sm" onClick={() => copy('uri', added.uri)}>
                    {copied === 'uri' ? t.asr_copied : t.asr_copy}
                  </button>
                </div>
              </div>
              <div style={{ fontSize: '13px', lineHeight: 1.8, wordBreak: 'break-all' }}>
                <div style={{ color: 'var(--system-text-secondary)' }}>{t.asr_manual}</div>
                <div>{t.asr_lan_label}{t.asr_colon}{added.lan || dash}</div>
                <div>{t.asr_wan_label}{t.asr_colon}{added.wan || dash}</div>
                <div>{t.asr_key}{t.asr_colon}<strong>{added.key}</strong></div>
                <div>{t.asr_fingerprint}{t.asr_colon}{added.fingerprint || dash}</div>
              </div>
              <div className="modal-hint">
                <span className="modal-hint-icon">ⓘ</span>
                <span>{t.asr_add_note}</span>
              </div>
            </div>
            <div className="modal-footer">
              <button className="btn btn-primary btn-accent" onClick={() => setAdded(null)}>{t.asr_close}</button>
            </div>
          </div>
        </div>
      )}

      {/* 撤销确认（不用浏览器原生 confirm） */}
      {revoking && (
        <div className="modal-overlay" onClick={() => setRevoking(null)}>
          <div className="modal-dialog modal-error" role="dialog" onClick={(e) => e.stopPropagation()}>
            <h3 className="modal-title">{t.asr_revoke}</h3>
            <div className="modal-body">
              <p>{fill(t.asr_revoke_confirm, { name: revoking.device_name || revoking.device_id })}</p>
            </div>
            <div className="modal-footer">
              <button className="btn btn-secondary" onClick={() => setRevoking(null)}>{t.asr_cancel}</button>
              <button
                className="btn btn-danger"
                data-testid="asr-revoke-confirm"
                onClick={async () => {
                  const id = revoking.id;
                  setRevoking(null);
                  await run('asr_server_revoke', { terminalId: id });
                }}
              >
                {t.asr_revoke}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};

export default AsrServerPage;
