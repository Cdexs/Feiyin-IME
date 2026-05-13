import React from 'react';
import { getTranslations } from '../i18n';

interface Props {
  config: any;
  updateConfig: (cfg: any) => void;
}

const GeneralPage: React.FC<Props> = ({ config, updateConfig }) => {
  const t = getTranslations(config.ui_language);

  const handleChange = (field: string, value: any) => {
    updateConfig({ ...config, [field]: value });
  };

  return (
    <div className="settings-page">
      <h2 className="page-title">{t.general_title}</h2>

      <section className="settings-section">
        <h3 className="section-title">{t.general_system_section}</h3>
        <label className="toggle-switch card" style={{ border: 'none', boxShadow: 'none', padding: '8px 0', marginBottom: 0 }}>
          <input
            type="checkbox"
            checked={config.auto_start}
            onChange={(e) => handleChange('auto_start', e.target.checked)}
            className="toggle-input"
          />
          <span className="toggle-track"></span>
          <span className="toggle-label">{t.general_auto_start}</span>
        </label>
      </section>

      <section className="settings-section">
        <h3 className="section-title">{t.general_ui_language}</h3>
        <select
          value={config.ui_language}
          onChange={(e) => handleChange('ui_language', e.target.value)}
          className="select-input"
        >
          <option value="Chinese">{t.general_ui_language_zh_hans}</option>
          <option value="TraditionalChinese">{t.general_ui_language_zh_hant}</option>
          <option value="English">{t.general_ui_language_en}</option>
        </select>
      </section>
    </div>
  );
};

export default GeneralPage;
