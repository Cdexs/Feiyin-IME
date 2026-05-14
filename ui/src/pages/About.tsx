import React, { useState, useEffect } from 'react';
import { getVersion } from '@tauri-apps/api/app';
import { invoke } from '@tauri-apps/api/core';
import { getTranslations } from '../i18n';

type Status = 'idle' | 'checking' | 'latest' | 'failed' | 'has_update';

interface VersionInfo {
  current: string;
  latest: string;
  url: string;
}

interface Props {
  config: any;
  updateConfig: (cfg: any) => void;
}

function isNewer(latest: string, current: string): boolean {
  const parse = (v: string) => v.replace(/^v/, '').split('.').map(n => parseInt(n, 10) || 0);
  const a = parse(latest), b = parse(current);
  for (let i = 0; i < Math.max(a.length, b.length); i++) {
    if ((a[i] || 0) > (b[i] || 0)) return true;
    if ((a[i] || 0) < (b[i] || 0)) return false;
  }
  return false;
}

const AboutPage: React.FC<Props> = ({ config }) => {
  const t = getTranslations(config.ui_language);
  const [version, setVersion] = useState('');
  const [latestVersion, setLatestVersion] = useState('');
  const [downloadUrl, setDownloadUrl] = useState('');
  const [status, setStatus] = useState<Status>('idle');

  useEffect(() => {
    getVersion().then(v => setVersion(v)).catch(() => setVersion(''));
    invoke<VersionInfo | null>('get_version_info')
      .then(res => {
        if (res) {
          setLatestVersion(res.latest);
          setDownloadUrl(res.url);
          if (isNewer(res.latest, res.current)) {
            setStatus('has_update');
          } else {
            setStatus('idle');
          }
        } else {
          setStatus('idle');
        }
      })
      .catch(() => setStatus('idle'));
  }, []);

  const handleCheck = async () => {
    setStatus('checking');
    try {
      const res = await invoke<VersionInfo | null>('force_check_latest_version');
      if (res) {
        setLatestVersion(res.latest);
        setDownloadUrl(res.url);
        if (isNewer(res.latest, res.current)) {
          setStatus('has_update');
        } else {
          setStatus('latest');
          setTimeout(() => setStatus('idle'), 3000);
        }
      } else {
        setStatus('failed');
        setTimeout(() => setStatus('idle'), 3000);
      }
    } catch {
      setStatus('failed');
      setTimeout(() => setStatus('idle'), 3000);
    }
  };

  const handleDownload = () => {
    if (downloadUrl) {
      invoke('open_url_in_browser', { url: downloadUrl }).catch(() => {});
    }
  };

  const buttonText = status === 'checking' ? t.about_checking : t.about_check_updates;
  const buttonDisabled = status === 'checking';

  return (
    <div className="settings-page" style={{ textAlign: 'center', paddingTop: '20px' }}>
      <div style={{
        width: '80px',
        height: '80px',
        margin: '0 auto 24px',
        borderRadius: 'var(--radius-large)',
        overflow: 'hidden',
        boxShadow: '0 4px 16px rgba(255, 107, 53, 0.25)'
      }}>
        <img
          src="/icons/icon-source.png"
          alt="Feiyin Logo"
          style={{ width: '100%', height: '100%', objectFit: 'contain' }}
        />
      </div>

      <h2 className="page-title" style={{ marginBottom: '8px' }}>{t.about_title}</h2>
      <p style={{ color: '#6b7280', marginBottom: '32px' }}>{t.about_subtitle}</p>

      <div className="card" style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', gap: '48px', minWidth: '240px', minHeight: '150px', margin: '0 auto' }}>
        <span style={{ color: '#6b7280', fontSize: '14px' }}>{t.about_version}</span>
        <span style={{ fontWeight: '500', fontSize: '14px' }}>{version ? `v${version}` : ''}</span>
      </div>

      {status === 'has_update' && (
        <div className="card" style={{ width: '380px', margin: '16px auto 0', textAlign: 'left' }}>
          <p style={{ fontSize: '14px', color: '#166534', marginBottom: '12px' }}>
            {t.about_new_version_found}{latestVersion}
          </p>
          <button className="btn btn-secondary" onClick={handleDownload} style={{ width: '100%' }}>
            {t.about_download_now}
          </button>
        </div>
      )}

      {status === 'latest' && (
        <p style={{ marginTop: '16px', fontSize: '14px', color: '#16a34a' }}>
          {t.about_up_to_date}
        </p>
      )}

      {status === 'failed' && (
        <p style={{ marginTop: '16px', fontSize: '14px', color: '#dc2626' }}>
          {t.about_check_failed}
        </p>
      )}

      <div style={{ marginTop: '24px' }}>
        <button className="btn btn-secondary" onClick={handleCheck} disabled={buttonDisabled} style={{ color: '#ff6b35', fontFamily: 'inherit' }}>
          {buttonText}
        </button>
      </div>
    </div>
  );
};

export default AboutPage;
