import React, { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { getTranslations } from '../i18n';

// STATS-475（Gavin 2026-10-04）：「我的」页 —— 本周（周一 ~ 周日）使用统计。
// 数据由主程序在每次输出最终文字时记录，这里只读汇总（get_usage_week）。

interface CategoryUsage {
  speech_ms: number;
  words: number;
}

export interface WeekUsage {
  week_start: string;
  week_end: string;
  speech_ms: number;
  words: number;
  llm_calls: number;
  local_fast: CategoryUsage;
  online_asr: CategoryUsage;
  local_streaming: CategoryUsage;
}

interface Props {
  config: any;
  updateConfig: (cfg: any) => void;
}

/** 毫秒 → 分钟（保留 1 位小数，整数不带 .0）。 */
export const formatMinutes = (ms: number): string => {
  const minutes = Math.round((ms / 60000) * 10) / 10;
  return Number.isInteger(minutes) ? String(minutes) : minutes.toFixed(1);
};

const MinePage: React.FC<Props> = ({ config }) => {
  const t = getTranslations(config.ui_language);
  const [usage, setUsage] = useState<WeekUsage | null>(null);
  const [loadFailed, setLoadFailed] = useState(false);

  useEffect(() => {
    invoke<WeekUsage>('get_usage_week')
      .then((data) => setUsage(data))
      .catch((e) => {
        console.error('Failed to load usage stats:', e);
        setLoadFailed(true);
      });
  }, []);

  // Gavin 2026-10-04「如果某一个分项的时长（或字数）是 0，也就是本周没用过，就不要显示这一项的统计信息」。
  const rows: Array<{ key: string; label: string; data: CategoryUsage }> = (usage
    ? [
        { key: 'local_fast', label: t.mine_local_fast, data: usage.local_fast },
        { key: 'online_asr', label: t.mine_online_asr, data: usage.online_asr },
        { key: 'local_streaming', label: t.mine_local_streaming, data: usage.local_streaming },
      ]
    : []
  ).filter((row) => row.data.speech_ms > 0 && row.data.words > 0);

  return (
    <div className="settings-page">
      <h2 className="page-title">{t.mine_title}</h2>

      <div className="card" data-testid="mine-week-usage">
        <div style={{ display: 'flex', alignItems: 'baseline', justifyContent: 'space-between', marginBottom: '16px' }}>
          <span style={{ fontSize: '15px', fontWeight: 600 }}>{t.mine_week_title}</span>
          {usage && (
            <span style={{ fontSize: '12px', color: '#6b7280' }}>
              {t.mine_week_range}{usage.week_start} ~ {usage.week_end}
            </span>
          )}
        </div>

        {loadFailed && (
          <p style={{ fontSize: '14px', color: 'var(--status-error)' }}>{t.mine_load_failed}</p>
        )}

        {usage && (
          <>
            <p data-testid="mine-total" style={{ fontSize: '14px', lineHeight: 1.8, margin: '0 0 12px' }}>
              {t.mine_total_duration}<strong>{formatMinutes(usage.speech_ms)}</strong>{t.mine_minutes}
              {t.mine_sep}
              {t.mine_input_words}<strong>{usage.words}</strong>{t.mine_words_unit}
              {t.mine_sep}
              {t.mine_llm_calls}<strong>{usage.llm_calls}</strong>{t.mine_times}
            </p>
            {rows.length > 0 && (
              <p style={{ fontSize: '14px', color: '#6b7280', margin: '0 0 6px' }}>{t.mine_breakdown}</p>
            )}
            <ul style={{ listStyle: 'none', margin: 0, padding: '0 0 0 16px' }}>
              {rows.map((row) => (
                <li key={row.key} data-testid={`mine-row-${row.key}`} style={{ fontSize: '14px', lineHeight: 2 }}>
                  <span style={{ display: 'inline-block', minWidth: '140px' }}>{row.label}</span>
                  {t.mine_duration}<strong>{formatMinutes(row.data.speech_ms)}</strong>{t.mine_minutes}
                  {t.mine_sep_inner}
                  {t.mine_input_words}<strong>{row.data.words}</strong>{t.mine_words_unit}
                </li>
              ))}
            </ul>
            <p style={{ fontSize: '12px', color: '#9ca3af', margin: '16px 0 0' }}>{t.mine_rules_note}</p>
          </>
        )}
      </div>
    </div>
  );
};

export default MinePage;
