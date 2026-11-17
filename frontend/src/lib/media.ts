import i18n from '@/lib/i18n';

/** getUserMedia reports the reason in `name`; each one needs a different thing from the person. */
export function micErrorLabel(err: unknown): string {
  const name = err instanceof DOMException ? err.name : '';
  switch (name) {
    case 'NotAllowedError':
    case 'SecurityError':
      return i18n.t('huddle:errors.micBlocked');
    case 'NotFoundError':
    case 'OverconstrainedError':
      return i18n.t('huddle:errors.micMissing');
    case 'NotReadableError':
    case 'AbortError':
      return i18n.t('huddle:errors.micBusy');
    default:
      return i18n.t('huddle:errors.micFailed');
  }
}

export async function acquireLocalAudio(deviceId?: string | null): Promise<MediaStream> {
  return navigator.mediaDevices.getUserMedia({
    audio: deviceId ? { deviceId: { exact: deviceId } } : true,
    video: false,
  });
}

export async function acquireCamera(deviceId?: string | null): Promise<MediaStream> {
  return navigator.mediaDevices.getUserMedia({
    audio: false,
    video: deviceId ? { deviceId: { exact: deviceId } } : true,
  });
}

export async function acquireScreen(): Promise<MediaStream> {
  return navigator.mediaDevices.getDisplayMedia({ video: true, audio: false });
}

export function stopStream(stream: MediaStream | null): void {
  stream?.getTracks().forEach((track) => track.stop());
}
