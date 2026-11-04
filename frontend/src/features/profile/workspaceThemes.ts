import type { CSSProperties } from 'react';
import type { components } from '@/api/schema';

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

export function sameTheme(a: Theme | null, b: Theme | null): boolean {
  if (!a || !b) return a === b;
  return (
    a.sidebar.toLowerCase() === b.sidebar.toLowerCase() &&
    a.selected.toLowerCase() === b.selected.toLowerCase() &&
    a.presence.toLowerCase() === b.presence.toLowerCase() &&
    a.badge.toLowerCase() === b.badge.toLowerCase()
  );
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
