import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, within } from '@testing-library/react';
import WorkspaceThemePicker from './WorkspaceThemePicker';
import { THEME_PRESETS, type Theme } from './workspaceThemes';

const mutate = vi.fn();
const preview = vi.fn();
let current: Theme | null = null;

vi.mock('@/stores/workspace', () => ({
  useWorkspaceStore: (select: (state: unknown) => unknown) =>
    select({ currentWorkspace: { id: 'w-1', name: 'Dev Team' } }),
}));
vi.mock('./hooks/useWorkspaceTheme', () => ({
  useSavedWorkspaceTheme: () => current,
  useSetWorkspaceTheme: () => ({ mutate, isError: false, isPending: false }),
  useThemePreview: (select: (state: unknown) => unknown) => select({ theme: null, preview }),
}));

const ocean = { sidebar: '#0c4a6e', selected: '#0284c7', presence: '#34d399', badge: '#f43f5e' };
const brand = { sidebar: '#102030', selected: '#d04020', presence: '#30c060', badge: '#e0a000' };

describe('WorkspaceThemePicker', () => {
  beforeEach(() => {
    mutate.mockReset();
    preview.mockReset();
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

  it('marks a theme that is no preset as custom', () => {
    current = brand;
    render(<WorkspaceThemePicker />);

    expect(screen.getByRole('radio', { name: 'Custom' })).toHaveAttribute('aria-checked', 'true');
    expect(screen.getByRole('radio', { name: 'Default' })).toHaveAttribute('aria-checked', 'false');
  });

  it('edits a custom theme from the saved one, previews it and saves it', () => {
    current = brand;
    render(<WorkspaceThemePicker />);

    fireEvent.click(screen.getByRole('radio', { name: 'Custom' }));
    const editor = within(screen.getByTestId('custom-theme'));
    expect(editor.getByLabelText('Sidebar')).toHaveValue('#102030');
    expect(preview).toHaveBeenLastCalledWith(brand);

    fireEvent.change(editor.getByLabelText('Selected item'), { target: { value: '#1164A3' } });
    expect(preview).toHaveBeenLastCalledWith({ ...brand, selected: '#1164a3' });

    fireEvent.click(editor.getByRole('button', { name: 'Save theme' }));
    expect(mutate).toHaveBeenCalledWith({ ...brand, selected: '#1164a3' });
    expect(screen.queryByTestId('custom-theme')).not.toBeInTheDocument();
    expect(preview).toHaveBeenLastCalledWith(null);
  });

  it('imports a shared line and refuses one that is not a theme', () => {
    render(<WorkspaceThemePicker />);
    fireEvent.click(screen.getByRole('radio', { name: 'Custom' }));
    const editor = within(screen.getByTestId('custom-theme'));
    const field = editor.getByLabelText('Import a theme');

    fireEvent.change(field, { target: { value: '#0c4a6e, #0284c7' } });
    fireEvent.click(editor.getByRole('button', { name: 'Import' }));
    expect(editor.getByRole('alert')).toHaveTextContent('That is not a theme');

    fireEvent.change(field, { target: { value: '#0c4a6e,#0284c7,#34d399,#f43f5e' } });
    fireEvent.keyDown(field, { key: 'Enter' });
    expect(editor.queryByRole('alert')).not.toBeInTheDocument();
    expect(editor.getByLabelText('Sidebar')).toHaveValue('#0c4a6e');
    expect(editor.getByTestId('custom-theme-share')).toHaveTextContent('#0c4a6e,#0284c7,#34d399,#f43f5e');
    expect(preview).toHaveBeenLastCalledWith(ocean);
  });

  it('keeps a half-typed colour out of the theme and says why', () => {
    render(<WorkspaceThemePicker />);
    fireEvent.click(screen.getByRole('radio', { name: 'Custom' }));
    const editor = within(screen.getByTestId('custom-theme'));

    fireEvent.change(editor.getByLabelText('Online'), { target: { value: '#2ba' } });
    expect(editor.getByRole('alert')).toHaveTextContent('Use a hex colour');
    expect(preview).toHaveBeenLastCalledWith(THEME_PRESETS[0].theme);
  });
});
