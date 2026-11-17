export type ClipMode = 'audio' | 'camera' | 'screen';
export type ClipContainer = 'mp4' | 'webm';
export type ClipKind = 'audio' | 'video';

export const CLIP_MAX_MS = 5 * 60 * 1000;
export const CLIP_COUNTDOWN_MS = 30 * 1000;

const RECORDING_TYPES: Record<ClipKind, string[]> = {
  video: [
    'video/mp4;codecs=avc1.42E01E,mp4a.40.2',
    'video/mp4;codecs=avc1,mp4a.40.2',
    'video/webm;codecs=vp8,opus',
    'video/webm;codecs=vp9,opus',
    'video/webm',
  ],
  audio: ['audio/mp4;codecs=mp4a.40.2', 'audio/webm;codecs=opus', 'audio/webm'],
};

export function kindOf(mode: ClipMode): ClipKind {
  return mode === 'audio' ? 'audio' : 'video';
}

export function recordingType(
  kind: ClipKind,
  supported: (type: string) => boolean = (type) => MediaRecorder.isTypeSupported(type),
): string | null {
  return RECORDING_TYPES[kind].find((type) => supported(type)) ?? null;
}

export function baseType(type: string): string {
  return type.split(';')[0].trim().toLowerCase();
}

export function containerOf(type: string): ClipContainer | null {
  const subtype = baseType(type).split('/')[1];
  return subtype === 'mp4' || subtype === 'webm' ? subtype : null;
}

export function clipFileName(kind: ClipKind, container: ClipContainer, at: Date): string {
  const pad = (n: number) => String(n).padStart(2, '0');
  const stamp = `${at.getFullYear()}-${pad(at.getMonth() + 1)}-${pad(at.getDate())} ${pad(at.getHours())}.${pad(at.getMinutes())}.${pad(at.getSeconds())}`;
  const extension = kind === 'audio' && container === 'mp4' ? 'm4a' : container;
  return `Clip ${stamp}.${extension}`;
}

export function clipClock(ms: number): string {
  const seconds = Math.max(0, Math.round(ms / 1000));
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, '0')}`;
}

export function canRecordClips(): boolean {
  return typeof MediaRecorder !== 'undefined' && typeof navigator.mediaDevices?.getUserMedia === 'function';
}
