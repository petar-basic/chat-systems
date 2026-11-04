import { describe, it, expect } from 'vitest';
import { AA, THEME_PRESETS, contrast, legible, mix, sameTheme, themeColours } from './workspaceThemes';

describe('workspace themes', () => {
  it.each(THEME_PRESETS.map((preset) => [preset.name, preset.theme] as const))(
    '%s keeps every sidebar text readable',
    (_name, theme) => {
      const colours = themeColours(theme);
      for (const text of ['--fg', '--fg-soft', '--fg-dim', '--muted', '--accent-soft']) {
        expect(contrast(colours[text], theme.sidebar), text).toBeGreaterThanOrEqual(AA);
      }
      expect(contrast(colours['--sidebar-selected-fg'], theme.selected)).toBeGreaterThanOrEqual(AA);
      expect(contrast(colours['--badge-fg'], theme.badge)).toBeGreaterThanOrEqual(AA);
    },
  );

  it('moves a text colour toward full strength until it passes', () => {
    const faint = mix('#3f0e40', '#ffffff', 0.3);
    expect(contrast(faint, '#3f0e40')).toBeLessThan(AA);

    const fixed = legible('#3f0e40', '#ffffff', 0.3, AA);
    expect(contrast(fixed, '#3f0e40')).toBeGreaterThanOrEqual(AA);
  });

  it('uses dark text on a light sidebar and light text on a dark one', () => {
    expect(
      themeColours({ sidebar: '#fef3c7', selected: '#f59e0b', presence: '#16a34a', badge: '#dc2626' })[
        '--fg'
      ],
    ).toBe('#0b1220');
    expect(
      themeColours({ sidebar: '#3f0e40', selected: '#1164a3', presence: '#2bac76', badge: '#cd2553' })[
        '--fg'
      ],
    ).toBe('#ffffff');
  });

  it('compares themes whatever the case of their colours', () => {
    const theme = { sidebar: '#3F0E40', selected: '#1164A3', presence: '#2BAC76', badge: '#CD2553' };
    expect(sameTheme(theme, THEME_PRESETS[0].theme)).toBe(true);
    expect(sameTheme(null, null)).toBe(true);
    expect(sameTheme(theme, null)).toBe(false);
  });
});
