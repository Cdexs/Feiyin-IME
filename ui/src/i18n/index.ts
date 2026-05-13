import { zhHans } from './zh-Hans';
import { zhHant } from './zh-Hant';
import { en } from './en';

export type TranslationKey = keyof typeof zhHans;
export type Locale = 'zh-Hans' | 'zh-Hant' | 'en';

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
