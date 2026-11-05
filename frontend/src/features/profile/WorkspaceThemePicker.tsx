import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Pipette } from 'lucide-react';
import { useWorkspaceStore } from '@/stores/workspace';
import CustomThemeEditor from './CustomThemeEditor';
import { useSavedWorkspaceTheme, useSetWorkspaceTheme } from './hooks/useWorkspaceTheme';
import { THEME_PRESETS, sameTheme, themeStyle, type Theme } from './workspaceThemes';

const OPTIONS: { id: string; name: string; theme: Theme | null }[] = [
  { id: 'default', name: 'Default', theme: null },
  ...THEME_PRESETS,
];

function ThemeSwatch({ theme }: { theme: Theme | null }) {
  return (
    <span
      aria-hidden="true"
      style={themeStyle(theme)}
      className="relative w-full h-10 rounded-md border border-line bg-sidebar flex flex-col justify-center gap-1 px-1.5"
    >
      <span className="h-1.5 rounded-sm bg-sidebar-selected" />
      <span className="h-1 w-3/4 rounded-sm bg-fg-dim/60" />
      <span className="absolute bottom-0.5 left-1 w-1.5 h-1.5 rounded-full bg-presence" />
      <span className="absolute top-0.5 right-0.5 w-1.5 h-1.5 rounded-full bg-badge" />
    </span>
  );
}

function optionStyle(checked: boolean): string {
  return `flex flex-col items-start gap-1.5 p-2 rounded-lg border text-xs font-medium transition cursor-pointer ${
    checked
      ? 'border-purple-500 bg-purple-600/15 text-fg'
      : 'border-line-strong bg-raised/50 text-muted hover:text-fg hover:bg-raised'
  }`;
}

export default function WorkspaceThemePicker() {
  const { t } = useTranslation('profile');
  const workspace = useWorkspaceStore((s) => s.currentWorkspace);
  const current = useSavedWorkspaceTheme();
  const setTheme = useSetWorkspaceTheme();
  const [editing, setEditing] = useState(false);
  if (!workspace) return null;

  const custom = current !== null && !THEME_PRESETS.some((preset) => sameTheme(current, preset.theme));
  const pick = (theme: Theme | null) => {
    setEditing(false);
    setTheme.mutate(theme);
  };

  return (
    <section className="mt-6" data-qa="workspace-theme">
      <h3 className="text-sm font-semibold text-fg">{t('workspaceTheme.title', { name: workspace.name })}</h3>
      <p className="mt-1 mb-3 text-sm text-muted">{t('workspaceTheme.hint')}</p>
      <div
        role="radiogroup"
        aria-label={t('workspaceTheme.title', { name: workspace.name })}
        className="grid grid-cols-2 sm:grid-cols-3 gap-2"
      >
        {OPTIONS.map(({ id, name, theme }) => {
          const checked = !editing && sameTheme(current, theme);
          return (
            <button
              key={id}
              type="button"
              role="radio"
              aria-checked={checked}
              data-qa={`workspace-theme-${id}`}
              onClick={() => pick(theme)}
              className={optionStyle(checked)}
            >
              <ThemeSwatch theme={theme} />
              <span className="truncate max-w-full">
                {t(`workspaceTheme.presets.${id}`, { defaultValue: name })}
              </span>
            </button>
          );
        })}
        <button
          type="button"
          role="radio"
          aria-checked={editing || custom}
          data-qa="workspace-theme-custom"
          onClick={() => setEditing(true)}
          className={optionStyle(editing || custom)}
        >
          {custom ? (
            <ThemeSwatch theme={current} />
          ) : (
            <span
              aria-hidden="true"
              className="w-full h-10 rounded-md border border-dashed border-line-strong flex items-center justify-center"
            >
              <Pipette className="w-4 h-4" />
            </span>
          )}
          <span className="truncate max-w-full">{t('workspaceTheme.custom')}</span>
        </button>
      </div>
      {editing && (
        <CustomThemeEditor
          initial={current ?? THEME_PRESETS[0].theme}
          saving={setTheme.isPending}
          onSave={pick}
          onCancel={() => setEditing(false)}
        />
      )}
      {setTheme.isError && (
        <p role="alert" className="mt-2 text-sm text-danger">
          {t('workspaceTheme.saveFailed')}
        </p>
      )}
    </section>
  );
}
