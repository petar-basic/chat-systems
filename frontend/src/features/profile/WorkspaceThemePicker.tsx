import { useWorkspaceStore } from '@/stores/workspace';
import { useSetWorkspaceTheme, useWorkspaceTheme } from './hooks/useWorkspaceTheme';
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

export default function WorkspaceThemePicker() {
  const workspace = useWorkspaceStore((s) => s.currentWorkspace);
  const current = useWorkspaceTheme();
  const setTheme = useSetWorkspaceTheme();
  if (!workspace) return null;

  return (
    <section className="mt-6" data-qa="workspace-theme">
      <h3 className="text-sm font-semibold text-fg">Theme for {workspace.name}</h3>
      <p className="mt-1 mb-3 text-sm text-muted">
        Colours the sidebar of this workspace. Only you see it, and each workspace has its own.
      </p>
      <div
        role="radiogroup"
        aria-label={`Theme for ${workspace.name}`}
        className="grid grid-cols-2 sm:grid-cols-4 gap-2"
      >
        {OPTIONS.map(({ id, name, theme }) => {
          const checked = sameTheme(current, theme);
          return (
            <button
              key={id}
              type="button"
              role="radio"
              aria-checked={checked}
              data-qa={`workspace-theme-${id}`}
              onClick={() => setTheme.mutate(theme)}
              className={`flex flex-col items-start gap-1.5 p-2 rounded-lg border text-xs font-medium transition cursor-pointer ${
                checked
                  ? 'border-purple-500 bg-purple-600/15 text-fg'
                  : 'border-line-strong bg-raised/50 text-muted hover:text-fg hover:bg-raised'
              }`}
            >
              <ThemeSwatch theme={theme} />
              <span className="truncate max-w-full">{name}</span>
            </button>
          );
        })}
      </div>
      {setTheme.isError && (
        <p role="alert" className="mt-2 text-sm text-danger">
          The theme could not be saved. Try again.
        </p>
      )}
    </section>
  );
}
