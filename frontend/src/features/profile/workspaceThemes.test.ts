import { describe, it, expect } from 'vitest';
import {
  AA,
  THEME_PRESETS,
  contrast,
  desktopColours,
  legible,
  mix,
  parseTheme,
  sameTheme,
  shareTheme,
  surpriseTheme,
  themeColours,
  unreadable,
} from './workspaceThemes';

const aubergine = THEME_PRESETS[0].theme;

function hue(hex: string): number {
  const [r, g, b] = [1, 3, 5].map((at) => parseInt(hex.slice(at, at + 2), 16) / 255);
  const max = Math.max(r, g, b);
  const spread = max - Math.min(r, g, b);
  if (spread === 0) return 0;
  const sector = max === r ? ((g - b) / spread) % 6 : max === g ? (b - r) / spread + 2 : (r - g) / spread + 4;
  return (sector * 60 + 360) % 360;
}

function hueDistance(a: string, b: string): number {
  const apart = Math.abs(hue(a) - hue(b));
  return Math.min(apart, 360 - apart);
}

function seeded(seed: number) {
  let state = seed;
  return () => {
    state = (state * 1664525 + 1013904223) % 2 ** 32;
    return state / 2 ** 32;
  };
}

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

  it('shares a theme as one line and reads it back', () => {
    expect(shareTheme(aubergine)).toBe('#3f0e40,#1164a3,#2bac76,#cd2553');
    expect(parseTheme(shareTheme(aubergine))).toEqual(aubergine);
    expect(parseTheme(' #3F0E40, 1164A3 ,#2bac76,#cd2553, ')).toEqual(aubergine);
  });

  it('reads a theme copied from Slack', () => {
    expect(
      parseTheme('#3F0E40,#350d36,#1164A3,#FFFFFF,#350D36,#FFFFFF,#2BAC76,#CD2553,#350d36,#FFFFFF'),
    ).toEqual(aubergine);
    expect(parseTheme('#3F0E40,#350d36,#1164A3,#FFFFFF,#350D36,#FFFFFF,#2BAC76,#CD2553')).toEqual(aubergine);
  });

  it.each([
    '',
    'aubergine',
    '#3f0e40,#1164a3,#2bac76',
    '#3f0e40,#1164a3,#2bac76,#cd2553,#ffffff',
    '#3f0e40,#1164a3,#2bac76,red',
    '#3f0,#1164a3,#2bac76,#cd2553',
  ])('refuses %j', (line) => {
    expect(parseTheme(line)).toBeNull();
  });

  it('names the colours whose text cannot be read', () => {
    expect(unreadable(aubergine)).toEqual([]);
    expect(unreadable({ ...aubergine, selected: '#888888' })).toEqual(['selected']);
  });

  it.each([1, 2, 3, 42, 1234, 99999])('surprises with readable colours (seed %i)', (seed) => {
    const theme = surpriseTheme(seeded(seed));
    expect(unreadable(theme)).toEqual([]);
    expect(contrast(theme.selected, theme.sidebar)).toBeGreaterThanOrEqual(1.5);
    expect(hueDistance(theme.selected, theme.badge)).toBeGreaterThanOrEqual(55);
    expect(parseTheme(shareTheme(theme))).toEqual(theme);
  });

  it('gives the desktop app the rail and badge colours and the mode they need', () => {
    expect(desktopColours(aubergine)).toEqual({
      mode: 'dark',
      rail: themeColours(aubergine)['--rail'],
      line: themeColours(aubergine)['--line'],
      fg: '#ffffff',
      muted: themeColours(aubergine)['--muted'],
      badge: '#cd2553',
      badge_fg: '#ffffff',
    });
    const banana = THEME_PRESETS.find((preset) => preset.id === 'banana')?.theme;
    expect(banana && desktopColours(banana).mode).toBe('light');
  });
});
