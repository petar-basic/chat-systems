import i18n from '@/lib/i18n';
import { acquireCamera, acquireLocalAudio, acquireScreen, micErrorLabel, stopStream } from '@/lib/media';
import { withBubble, type Composed } from './compose';
import { baseType, kindOf, recordingType, type ClipMode } from './format';
import { tick } from './ticker';
import { levelOf } from './waveform';

const LEVEL_EVERY_MS = 100;
const SLICE_MS = 1000;
const BITRATE: Record<ClipMode, number> = { audio: 0, camera: 1_200_000, screen: 2_000_000 };
const AUDIO_BITRATE = 96_000;

export class ClipDenied extends Error {}

export interface Recorded {
  recording: Blob;
  type: string;
  levels: number[];
  elapsedMs: number;
}

function deniedName(err: unknown): string {
  return err instanceof DOMException ? err.name : '';
}

function cameraError(err: unknown): ClipDenied {
  const name = deniedName(err);
  const key = name === 'NotFoundError' || name === 'OverconstrainedError' ? 'cameraMissing' : 'cameraDenied';
  return new ClipDenied(i18n.t(`messages:clips.errors.${key}`));
}

async function open<T>(
  get: () => Promise<T>,
  fail: (err: unknown) => ClipDenied,
  opened: MediaStream[],
): Promise<T> {
  try {
    return await get();
  } catch (err) {
    opened.forEach((stream) => stopStream(stream));
    throw fail(err);
  }
}

export class ClipSession {
  readonly preview: MediaStream | null;
  private readonly chunks: Blob[] = [];
  private readonly levels: number[] = [];
  private readonly startedAt = performance.now();
  private stopLevels: (() => void) | null = null;
  private audioContext: AudioContext | null = null;
  private finished: Promise<Recorded> | null = null;

  private constructor(
    private readonly recorder: MediaRecorder,
    private readonly sources: MediaStream[],
    private readonly composed: Composed | null,
    microphone: MediaStream,
    measure: boolean,
  ) {
    const video = recorder.stream.getVideoTracks();
    this.preview = video.length > 0 ? new MediaStream(video) : null;
    recorder.ondataavailable = (event) => {
      if (event.data.size > 0) this.chunks.push(event.data);
    };
    if (measure) this.measure(microphone);
    recorder.start(SLICE_MS);
  }

  static async start(mode: ClipMode, bubble: boolean, onEnded: () => void): Promise<ClipSession> {
    const type = recordingType(kindOf(mode));
    if (!type) throw new ClipDenied(i18n.t('messages:clips.errors.unsupported'));
    const opened: MediaStream[] = [];
    const screenDenied = () => new ClipDenied(i18n.t('messages:clips.errors.screenDenied'));
    const screen = mode === 'screen' ? await open(acquireScreen, screenDenied, opened) : null;
    if (screen) opened.push(screen);
    const microphone = await open(
      () => acquireLocalAudio(),
      (err) => new ClipDenied(micErrorLabel(err)),
      opened,
    );
    opened.push(microphone);
    const camera =
      mode === 'camera' || (mode === 'screen' && bubble)
        ? await open(() => acquireCamera(), cameraError, opened)
        : null;
    if (camera) opened.push(camera);

    const composed = screen && camera ? withBubble(screen, camera) : null;
    const video = composed?.stream ?? screen ?? camera;
    const tracks = [...(video?.getVideoTracks() ?? []), ...microphone.getAudioTracks()];
    let recorder: MediaRecorder;
    try {
      recorder = new MediaRecorder(new MediaStream(tracks), {
        mimeType: type,
        audioBitsPerSecond: AUDIO_BITRATE,
        ...(mode === 'audio' ? {} : { videoBitsPerSecond: BITRATE[mode] }),
      });
    } catch {
      composed?.stop();
      opened.forEach((stream) => stopStream(stream));
      throw new ClipDenied(i18n.t('messages:clips.errors.unsupported'));
    }
    screen?.getVideoTracks()[0]?.addEventListener('ended', onEnded, { once: true });
    return new ClipSession(recorder, opened, composed, microphone, mode === 'audio');
  }

  get elapsedMs(): number {
    return performance.now() - this.startedAt;
  }

  stop(): Promise<Recorded> {
    if (this.finished) return this.finished;
    const elapsedMs = this.elapsedMs;
    this.finished = new Promise<Recorded>((resolve, reject) => {
      this.recorder.onstop = () => {
        const type = baseType(this.recorder.mimeType || this.chunks[0]?.type || '');
        this.release();
        resolve({ recording: new Blob(this.chunks, { type }), type, levels: [...this.levels], elapsedMs });
      };
      this.recorder.onerror = () => {
        this.release();
        reject(new ClipDenied(i18n.t('messages:clips.errors.failed')));
      };
    });
    if (this.recorder.state === 'inactive') this.recorder.onstop?.(new Event('stop'));
    else this.recorder.stop();
    return this.finished;
  }

  cancel(): void {
    this.recorder.ondataavailable = null;
    this.recorder.onstop = null;
    if (this.recorder.state !== 'inactive') this.recorder.stop();
    this.release();
  }

  private measure(microphone: MediaStream) {
    try {
      const context = new AudioContext();
      const analyser = context.createAnalyser();
      analyser.fftSize = 1024;
      context.createMediaStreamSource(microphone).connect(analyser);
      const samples = new Uint8Array(analyser.fftSize);
      this.audioContext = context;
      this.stopLevels = tick(LEVEL_EVERY_MS, () => {
        analyser.getByteTimeDomainData(samples);
        this.levels.push(levelOf(samples));
      });
    } catch {
      this.stopLevels = null;
    }
  }

  private release() {
    this.stopLevels?.();
    this.stopLevels = null;
    void this.audioContext?.close().catch(() => undefined);
    this.audioContext = null;
    this.composed?.stop();
    this.sources.forEach((stream) => stopStream(stream));
  }
}
