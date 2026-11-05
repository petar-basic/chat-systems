import { useTranslation } from 'react-i18next';
import { Monitor, Moon, Sun } from 'lucide-react';
import { useThemeStore, type ThemeMode } from '@/stores/theme';
import { useUpdatePreferences } from './hooks/usePreferences';

const OPTIONS: { mode: ThemeMode; Icon: typeof Sun }[] = [
  { mode: 'light', Icon: Sun },
  { mode: 'dark', Icon: Moon },
  { mode: 'system', Icon: Monitor },
];

export default function ThemePicker() {
  const { t } = useTranslation('profile');
  const mode = useThemeStore((s) => s.mode);
  const setMode = useThemeStore((s) => s.setMode);
  const update = useUpdatePreferences();
  const pick = (next: ThemeMode) => {
    setMode(next);
    update.mutate({ colour_mode: next });
  };

  return (
    <div
      role="radiogroup"
      aria-label={t('theme.label')}
      data-qa="theme-picker"
      className="grid grid-cols-3 gap-2"
    >
      {OPTIONS.map(({ mode: value, Icon }) => (
        <button
          key={value}
          type="button"
          role="radio"
          aria-checked={mode === value}
          data-qa={`theme-option-${value}`}
          onClick={() => pick(value)}
          className={`flex flex-col items-center gap-1.5 px-3 py-3 rounded-lg border text-xs font-medium transition cursor-pointer ${
            mode === value
              ? 'border-purple-500 bg-purple-600/15 text-fg'
              : 'border-line-strong bg-raised/50 text-muted hover:text-fg hover:bg-raised'
          }`}
        >
          <Icon className="w-4 h-4" />
          {t(`theme.${value}`)}
        </button>
      ))}
    </div>
  );
}
