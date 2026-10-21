export function linkPreviewsEnabled(settings: unknown): boolean {
  if (typeof settings !== 'object' || settings === null) return true;
  return (settings as Record<string, unknown>).link_previews !== false;
}
