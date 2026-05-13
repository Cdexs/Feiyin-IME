import React, { useState, useEffect } from 'react';
import { getVersion } from '@tauri-apps/api/app';
import { getTranslations } from '../i18n';

interface Props {
  config: any;
  updateConfig: (cfg: any) => void;
}

const AboutPage: React.FC<Props> = ({ config }) => {
  const t = getTranslations(config.ui_language);
  const [version, setVersion] = useState('');

  useEffect(() => {
    getVersion().then(v => setVersion(v)).catch(() => setVersion(''));
  }, []);

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

      <div className="card" style={{ display: 'flex', flexDirection: 'column', width: '280px', margin: '0 auto', textAlign: 'left' }}>
        <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: '12px', fontSize: '14px' }}>
          <span style={{ color: '#6b7280' }}>{t.about_version}</span>
          <span style={{ fontWeight: '500' }}>{version ? `v${version}` : ''}</span>
        </div>
      </div>

      <div style={{ marginTop: '40px' }}>
        <button className="btn btn-secondary">
          {t.about_check_updates}
        </button>
      </div>
    </div>
  );
};

export default AboutPage;
