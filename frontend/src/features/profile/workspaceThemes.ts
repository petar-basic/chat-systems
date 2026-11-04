import type { CSSProperties } from 'react';
import type { components } from '@/api/schema';
import type { DesktopColours } from '@/lib/desktopShell';

export type Theme = components['schemas']['Theme'];

export interface ThemePreset {
  id: string;
  name: string;
  theme: Theme;
}

export const THEME_PRESETS: ThemePreset[] = [
  {
    id: 'aubergine',
    name: 'Aubergine',
    theme: { sidebar: '#3f0e40', selected: '#1164a3', presence: '#2bac76', badge: '#cd2553' },
  },
  {
    id: 'midnight',
    name: 'Midnight',
    theme: { sidebar: '#0f172a', selected: '#4f46e5', presence: '#22c55e', badge: '#ef4444' },
  },
  {
    id: 'ocean',
    name: 'Ocean',
    theme: { sidebar: '#0c4a6e', selected: '#0284c7', presence: '#34d399', badge: '#f43f5e' },
  },
  {
    id: 'forest',
    name: 'Forest',
    theme: { sidebar: '#13392b', selected: '#2d7a57', presence: '#4ade80', badge: '#e11d48' },
  },
  {
    id: 'clementine',
    name: 'Clementine',
    theme: { sidebar: '#7c2d12', selected: '#ea580c', presence: '#4ade80', badge: '#facc15' },
  },
  {
    id: 'banana',
    name: 'Banana',
    theme: { sidebar: '#fef3c7', selected: '#f59e0b', presence: '#16a34a', badge: '#dc2626' },
  },
  {
    id: 'graphite',
    name: 'Graphite',
    theme: { sidebar: '#27272a', selected: '#52525b', presence: '#22c55e', badge: '#ef4444' },
  },
];

const LIGHT_TEXT = '#ffffff';
const DARK_TEXT = '#0b1220';
export const AA = 4.5;
const DIM = 3;

function channels(hex: string): [number, number, number] {
  const value = parseInt(hex.slice(1), 16);
  return [(value >> 16) & 255, (value >> 8) & 255, value & 255];
}

export function mix(from: string, to: string, share: number): string {
  const a = channels(from);
  const b = channels(to);
  return `#${a
    .map((channel, i) => Math.round(channel + (b[i] - channel) * share))
    .map((channel) => channel.toString(16).padStart(2, '0'))
    .join('')}`;
}

function luminance(hex: string): number {
  const [r, g, b] = channels(hex).map((channel) => {
    const value = channel / 255;
    return value <= 0.03928 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

export function contrast(a: string, b: string): number {
  const [light, dark] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (light + 0.05) / (dark + 0.05);
}

export function readableOn(background: string): string {
  return contrast(LIGHT_TEXT, background) >= contrast(DARK_TEXT, background) ? LIGHT_TEXT : DARK_TEXT;
}

export function legible(background: string, text: string, share: number, ratio: number): string {
  for (let step = share; step < 1; step += 0.05) {
    const colour = mix(background, text, step);
    if (contrast(colour, background) >= ratio) return colour;
  }
  return text;
}

export const THEME_KEYS = ['sidebar', 'selected', 'presence', 'badge'] as const;
export type ThemeKey = (typeof THEME_KEYS)[number];

export function sameTheme(a: Theme | null, b: Theme | null): boolean {
  if (!a || !b) return a === b;
  return THEME_KEYS.every((key) => a[key].toLowerCase() === b[key].toLowerCase());
}

export function isHexColour(value: string): boolean {
  return /^#[0-9a-f]{6}$/i.test(value);
}

export function shareTheme(theme: Theme): string {
  return THEME_KEYS.map((key) => theme[key].toLowerCase()).join(',');
}

const SLACK_SLOTS: Record<ThemeKey, number> = { sidebar: 0, selected: 2, presence: 6, badge: 7 };

export function parseTheme(line: string): Theme | null {
  const parts = line
    .trim()
    .replace(/,$/, '')
    .split(',')
    .map((part) => part.trim());
  if (!parts.every((part) => /^#?[0-9a-f]{6}$/i.test(part))) return null;
  const colours = parts.map((part) => `#${part.replace('#', '').toLowerCase()}`);
  const at = (slots: Record<ThemeKey, number>): Theme => ({
    sidebar: colours[slots.sidebar],
    selected: colours[slots.selected],
    presence: colours[slots.presence],
    badge: colours[slots.badge],
  });
  if (colours.length === THEME_KEYS.length) return at({ sidebar: 0, selected: 1, presence: 2, badge: 3 });
  if (colours.length === 8 || colours.length === 10) return at(SLACK_SLOTS);
  return null;
}

export function unreadable(theme: Theme): ThemeKey[] {
  return (['sidebar', 'selected', 'badge'] as const).filter(
    (key) => contrast(readableOn(theme[key]), theme[key]) < AA,
  );
}

function hsl(hue: number, saturation: number, lightness: number): string {
  const k = (n: number) => (n + hue / 30) % 12;
  const a = saturation * Math.min(lightness, 1 - lightness);
  const channel = (n: number) =>
    Math.round(255 * (lightness - a * Math.max(-1, Math.min(k(n) - 3, 9 - k(n), 1))))
      .toString(16)
      .padStart(2, '0');
  return `#${channel(0)}${channel(8)}${channel(4)}`;
}

function hueDistance(a: number, b: number): number {
  const apart = Math.abs(a - b) % 360;
  return Math.min(apart, 360 - apart);
}

export function surpriseTheme(random: () => number = Math.random): Theme {
  for (let attempt = 0; attempt < 50; attempt += 1) {
    const hue = random() * 360;
    const dark = random() < 0.75;
    const selectedHue = (hue + 40 + random() * 140) % 360;
    const badgeHue = (340 + random() * 30) % 360;
    const theme = {
      sidebar: hsl(hue, 0.3 + random() * 0.4, dark ? 0.1 + random() * 0.12 : 0.88 + random() * 0.06),
      selected: hsl(selectedHue, 0.55 + random() * 0.3, dark ? 0.38 : 0.42),
      presence: hsl(130 + random() * 30, 0.65, dark ? 0.5 : 0.36),
      badge: hsl(badgeHue, 0.75, 0.45),
    };
    if (
      unreadable(theme).length === 0 &&
      contrast(theme.selected, theme.sidebar) >= 1.5 &&
      hueDistance(selectedHue, badgeHue) >= 60
    )
      return theme;
  }
  return THEME_PRESETS[0].theme;
}

export function themeColours(theme: Theme): Record<string, string> {
  const fg = readableOn(theme.sidebar);
  const tint = (share: number) => mix(theme.sidebar, fg, share);
  return {
    '--sidebar': theme.sidebar,
    '--surface': theme.sidebar,
    '--rail': mix(theme.sidebar, '#000000', 0.3),
    '--raised': tint(0.14),
    '--elevated': tint(0.22),
    '--line': tint(0.16),
    '--line-strong': tint(0.28),
    '--fg': fg,
    '--fg-soft': legible(theme.sidebar, fg, 0.92, AA),
    '--fg-dim': legible(theme.sidebar, fg, 0.85, AA),
    '--muted': legible(theme.sidebar, fg, 0.72, AA),
    '--subtle': legible(theme.sidebar, fg, 0.58, DIM),
    '--faint': tint(0.42),
    '--accent': fg,
    '--accent-soft': legible(theme.sidebar, fg, 0.85, AA),
    '--sidebar-selected': theme.selected,
    '--sidebar-selected-fg': readableOn(theme.selected),
    '--presence': theme.presence,
    '--badge': theme.badge,
    '--badge-fg': readableOn(theme.badge),
  };
}

export function themeStyle(theme: Theme | null): CSSProperties | undefined {
  return theme ? (themeColours(theme) as CSSProperties) : undefined;
}

export function desktopColours(theme: Theme): DesktopColours {
  const colours = themeColours(theme);
  return {
    mode: colours['--fg'] === LIGHT_TEXT ? 'dark' : 'light',
    rail: colours['--rail'],
    line: colours['--line'],
    fg: colours['--fg'],
    muted: colours['--muted'],
    badge: colours['--badge'],
    badge_fg: colours['--badge-fg'],
  };
}
