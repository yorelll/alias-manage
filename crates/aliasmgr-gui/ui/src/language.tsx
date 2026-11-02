import { createContext, useContext } from "react";
import type { Language, TranslationKey } from "./i18n";
import { DEFAULT_LANGUAGE, t as translate } from "./i18n";

export interface I18nContextValue {
  language: Language;
  setLanguage: (language: Language) => void;
  t: (key: TranslationKey, params?: Record<string, string | number>) => string;
}

/**
 * Default context keeps components renderable in isolation: Chinese until the
 * application shell loads the persisted setting from `config_get`.
 */
export const I18nContext = createContext<I18nContextValue>({
  language: DEFAULT_LANGUAGE,
  setLanguage: () => {},
  t: (key, params) => translate(DEFAULT_LANGUAGE, key, params),
});

export function useI18n(): I18nContextValue {
  return useContext(I18nContext);
}
