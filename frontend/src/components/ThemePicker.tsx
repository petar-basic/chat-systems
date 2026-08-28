import { Monitor, Moon, Sun } from 'lucide-react';
import { useThemeStore, type ThemeMode } from '@/stores/theme';

const OPTIONS: { mode: ThemeMode; label: string; Icon: typeof Sun }[] = [
  { mode: 'light', label: 'Light', Icon: Sun },
  { mode: 'dark', label: 'Dark', Icon: Moon },
  { mode: 'system', label: 'System', Icon: Monitor },
];

export default function ThemePicker() {
  const mode = useThemeStore((s) => s.mode);
  const setMode = useThemeStore((s) => s.setMode);

  return (
    <div role="radiogroup" aria-label="Theme" data-qa="theme-picker" className="grid grid-cols-3 gap-2">
      {OPTIONS.map(({ mode: value, label, Icon }) => (
        <button
          key={value}
          type="button"
          role="radio"
          aria-checked={mode === value}
          data-qa={`theme-option-${value}`}
          onClick={() => setMode(value)}
          className={`flex flex-col items-center gap-1.5 px-3 py-3 rounded-lg border text-xs font-medium transition cursor-pointer ${
            mode === value
              ? 'border-purple-500 bg-purple-600/15 text-fg'
              : 'border-line-strong bg-raised/50 text-muted hover:text-fg hover:bg-raised'
          }`}
        >
          <Icon className="w-4 h-4" />
          {label}
        </button>
      ))}
    </div>
  );
}
