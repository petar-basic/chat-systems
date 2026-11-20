export const PREFERENCES_SECTIONS = [
  'notifications',
  'appearance',
  'messages',
  'region',
  'read',
  'account',
  'security',
  'calendar',
  'advanced',
] as const;

export type PreferencesSection = (typeof PREFERENCES_SECTIONS)[number];

export function preferencesSectionOf(raw: string | null): PreferencesSection | undefined {
  return PREFERENCES_SECTIONS.find((section) => section === raw);
}
