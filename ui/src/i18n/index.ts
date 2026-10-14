import { zhHans } from './zh-Hans';
import { zhHant } from './zh-Hant';
import { en } from './en';

export type TranslationKey = keyof typeof zhHans;
export type Locale = 'zh-Hans' | 'zh-Hant' | 'en';

/** 配置还没读到时（启动加载 / 读取失败）按系统语言选文案，取值与配置的 ui_language 同口径。 */
export function uiLanguageFromSystem(lang: string = typeof navigator !== 'undefined' ? navigator.language : ''): string {
  const l = (lang || '').toLowerCase();
  if (/^zh-(tw|hk|mo|hant)/.test(l)) return 'TraditionalChinese';
  if (l.startsWith('zh')) return 'Chinese';
  return 'English';
}

export function getTranslations(uiLanguage: string): Record<TranslationKey, string> {
  switch (uiLanguage) {
    case 'Chinese':
      return zhHans as unknown as Record<TranslationKey, string>;
    case 'TraditionalChinese':
      return zhHant as unknown as Record<TranslationKey, string>;
    default:
      return en as unknown as Record<TranslationKey, string>;
  }
}
