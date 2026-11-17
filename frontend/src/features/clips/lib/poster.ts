const POSTER_WIDTH = 1280;
const POSTER_AT_S = 0.5;
const POSTER_TIMEOUT_MS = 8000;
const FALLBACK = { width: 1280, height: 720 };

function settle<T>(element: HTMLMediaElement, event: string, value: () => T): Promise<T> {
  return new Promise((resolve, reject) => {
    const done = () => {
      element.removeEventListener('error', failed);
      resolve(value());
    };
    const failed = () => {
      element.removeEventListener(event, done);
      reject(new Error(`the recording did not ${event}`));
    };
    element.addEventListener(event, done, { once: true });
    element.addEventListener('error', failed, { once: true });
  });
}

function toJpeg(canvas: HTMLCanvasElement): Promise<Blob> {
  return new Promise((resolve, reject) =>
    canvas.toBlob((blob) => (blob ? resolve(blob) : reject(new Error('no poster'))), 'image/jpeg', 0.82),
  );
}

async function frameOf(recording: Blob): Promise<Blob> {
  const url = URL.createObjectURL(recording);
  const video = document.createElement('video');
  video.muted = true;
  video.playsInline = true;
  video.preload = 'auto';
  try {
    const loaded = settle(video, 'loadeddata', () => undefined);
    video.src = url;
    await loaded;
    const at = Number.isFinite(video.duration) ? Math.min(POSTER_AT_S, video.duration / 2) : 0;
    if (at > 0) {
      const seeked = settle(video, 'seeked', () => undefined);
      video.currentTime = at;
      await seeked;
    }
    const scale = Math.min(1, POSTER_WIDTH / (video.videoWidth || POSTER_WIDTH));
    const canvas = document.createElement('canvas');
    canvas.width = Math.max(1, Math.round((video.videoWidth || FALLBACK.width) * scale));
    canvas.height = Math.max(1, Math.round((video.videoHeight || FALLBACK.height) * scale));
    canvas.getContext('2d')?.drawImage(video, 0, 0, canvas.width, canvas.height);
    return await toJpeg(canvas);
  } finally {
    video.removeAttribute('src');
    video.load();
    URL.revokeObjectURL(url);
  }
}

function blank(): Promise<Blob> {
  const canvas = document.createElement('canvas');
  canvas.width = FALLBACK.width;
  canvas.height = FALLBACK.height;
  const ctx = canvas.getContext('2d');
  if (ctx) {
    ctx.fillStyle = '#1e293b';
    ctx.fillRect(0, 0, canvas.width, canvas.height);
  }
  return toJpeg(canvas);
}

export async function posterOf(recording: Blob): Promise<Blob> {
  const timeout = new Promise<never>((_, reject) =>
    window.setTimeout(() => reject(new Error('the poster took too long')), POSTER_TIMEOUT_MS),
  );
  try {
    return await Promise.race([frameOf(recording), timeout]);
  } catch {
    return blank();
  }
}
