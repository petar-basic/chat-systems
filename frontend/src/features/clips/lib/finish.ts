import type { components } from '@/api/schema';
import { logger } from '@/lib/logger';
import { CLIP_MAX_MS, baseType, clipFileName, containerOf, kindOf, type ClipMode } from './format';
import { posterOf } from './poster';
import { makeSeekable } from './remux';
import type { Recorded } from './session';
import { peaksOf } from './waveform';

export type ClipMeta = components['schemas']['Clip'];

export interface RecordedClip {
  file: File;
  poster: Blob | null;
  previewUrl: string;
  posterUrl: string | null;
  clip: ClipMeta;
}

export async function finishClip(recorded: Recorded, mode: ClipMode, at: Date): Promise<RecordedClip> {
  const kind = kindOf(mode);
  const type = baseType(recorded.type);
  const container = containerOf(type) ?? 'webm';
  const seekable = await makeSeekable(recorded.recording, container).catch((err: unknown) => {
    logger.warn('clips', 'finishClip', `kept the recording as it was: ${String(err)}`);
    return { blob: recorded.recording, durationMs: 0 };
  });
  const durationMs = Math.min(
    CLIP_MAX_MS,
    Math.max(1, Math.round(seekable.durationMs || recorded.elapsedMs)),
  );
  const file = new File([seekable.blob], clipFileName(kind, container, at), { type });
  const poster = kind === 'video' ? await posterOf(file) : null;
  const clip: ClipMeta =
    kind === 'audio'
      ? { kind, duration_ms: durationMs, waveform: peaksOf(recorded.levels) }
      : { kind, duration_ms: durationMs };
  return {
    file,
    poster,
    previewUrl: URL.createObjectURL(file),
    posterUrl: poster ? URL.createObjectURL(poster) : null,
    clip,
  };
}

export function releaseClip(clip: RecordedClip): void {
  URL.revokeObjectURL(clip.previewUrl);
  if (clip.posterUrl) URL.revokeObjectURL(clip.posterUrl);
}
