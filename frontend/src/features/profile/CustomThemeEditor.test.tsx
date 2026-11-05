import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import i18n from '@/lib/i18n';
import CustomThemeEditor from './CustomThemeEditor';
import { unreadable } from './workspaceThemes';

vi.mock('./hooks/useWorkspaceTheme', () => ({
  useThemePreview: (select: (state: unknown) => unknown) => select({ preview: vi.fn() }),
}));

const grey = { sidebar: '#777777', selected: '#1164a3', presence: '#2bac76', badge: '#777777' };

describe('CustomThemeEditor', () => {
  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('in Japanese lists the parts that will be hard to read', async () => {
    expect(unreadable(grey)).toEqual(['sidebar', 'badge']);
    await i18n.changeLanguage('ja');
    render(<CustomThemeEditor initial={grey} saving={false} onSave={vi.fn()} onCancel={vi.fn()} />);

    expect(screen.getByTestId('custom-theme-unreadable')).toHaveTextContent(
      'サイドバー、メンションのバッジの文字が読みにくくなります。',
    );
    expect(screen.getByRole('button', { name: 'テーマを保存' })).toBeInTheDocument();
  });

  it('in English joins them with "and"', () => {
    render(<CustomThemeEditor initial={grey} saving={false} onSave={vi.fn()} onCancel={vi.fn()} />);

    expect(screen.getByTestId('custom-theme-unreadable')).toHaveTextContent(
      'Text on the sidebar and mention badge will be hard to read.',
    );
  });
});
