import { desktopShell } from '@/lib/desktopShell';
import { displayNameOf } from '@/lib/userHelpers';
import { useUserCache } from '@/stores/users';
import { DRAW_FADE_MS, drawBoard, fadeStartOf, strokeColorFor } from './drawing';

export function mirrorOnDesktop(track: MediaStreamTrack, sharerId: string): () => void {
  const shell = desktopShell();
  const { openDrawOverlay, drawOnOverlay, closeDrawOverlay } = shell ?? {};
  const settings = track.getSettings();
  if (!openDrawOverlay || !drawOnOverlay || !closeDrawOverlay || settings.displaySurface !== 'monitor') {
    return () => undefined;
  }
  openDrawOverlay(Math.round(settings.width ?? 0), Math.round(settings.height ?? 0));
  const off = drawBoard.subscribe((id, stroke) => {
    if (id !== sharerId || !stroke) return;
    drawOnOverlay({
      key: stroke.key,
      color: strokeColorFor(stroke.userId),
      name: displayNameOf(useUserCache.getState().getUser(stroke.userId)?.display_name),
      points: stroke.points,
      fade_at: fadeStartOf(stroke),
      fade_ms: DRAW_FADE_MS,
    });
  });
  return () => {
    off();
    closeDrawOverlay();
  };
}
