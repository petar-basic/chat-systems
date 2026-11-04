import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import WorkspaceThemePicker from './WorkspaceThemePicker';
import { THEME_PRESETS, type Theme } from './workspaceThemes';

const mutate = vi.fn();
let current: Theme | null = null;

vi.mock('@/stores/workspace', () => ({
  useWorkspaceStore: (select: (state: unknown) => unknown) =>
    select({ currentWorkspace: { id: 'w-1', name: 'Dev Team' } }),
}));
vi.mock('./hooks/useWorkspaceTheme', () => ({
  useWorkspaceTheme: () => current,
  useSetWorkspaceTheme: () => ({ mutate, isError: false }),
}));

describe('WorkspaceThemePicker', () => {
  beforeEach(() => {
    mutate.mockReset();
    current = null;
  });

  it('says the theme is for this workspace and marks the default when none is set', () => {
    render(<WorkspaceThemePicker />);

    expect(screen.getByRole('radiogroup', { name: 'Theme for Dev Team' })).toBeInTheDocument();
    expect(screen.getByRole('radio', { name: 'Default' })).toHaveAttribute('aria-checked', 'true');
    expect(screen.getByRole('radio', { name: 'Aubergine' })).toHaveAttribute('aria-checked', 'false');
  });

  it('marks the saved theme and saves another one on click', () => {
    current = THEME_PRESETS[0].theme;
    render(<WorkspaceThemePicker />);

    expect(screen.getByRole('radio', { name: 'Aubergine' })).toHaveAttribute('aria-checked', 'true');

    fireEvent.click(screen.getByRole('radio', { name: 'Forest' }));
    expect(mutate).toHaveBeenCalledWith(THEME_PRESETS.find((preset) => preset.id === 'forest')?.theme);

    fireEvent.click(screen.getByRole('radio', { name: 'Default' }));
    expect(mutate).toHaveBeenLastCalledWith(null);
  });
});
