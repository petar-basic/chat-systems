import { useTranslation } from 'react-i18next';
import type { components } from '@/api/schema';
import { Select } from '@/shared/components/Form/Select';
import { LANGUAGES, LANGUAGE_NAMES, isLanguage } from '@/lib/i18n';
import { usePreferences, useUpdatePreferences } from './hooks/usePreferences';

type Choice = components['schemas']['Language'];

export default function LanguagePicker() {
  const { t } = useTranslation('profile');
  const { data } = usePreferences();
  const update = useUpdatePreferences();

  const saved = data?.language;
  const current: Choice = isLanguage(saved) ? saved : 'browser';

  return (
    <section data-qa="language">
      <h3 className="text-sm font-semibold text-fg">{t('language.label')}</h3>
      <p className="mt-1 mb-3 text-sm text-muted">{t('language.hint')}</p>
      <Select
        label={t('language.label')}
        labelHidden
        value={current}
        onChange={(e) => {
          const language = e.target.value;
          if (language === 'browser' || isLanguage(language)) update.mutate({ language });
        }}
        disabled={!data}
        dataQa="language-picker"
      >
        <option value="browser">{t('language.browser')}</option>
        {LANGUAGES.map((language) => (
          <option key={language} value={language} lang={language}>
            {LANGUAGE_NAMES[language]}
          </option>
        ))}
      </Select>
      {update.isError && (
        <p role="alert" className="mt-2 text-sm text-danger">
          {t('language.saveFailed')}
        </p>
      )}
    </section>
  );
}
