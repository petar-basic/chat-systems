import { FilesetResolver, ImageSegmenter, type ImageSegmenterResult } from '@mediapipe/tasks-vision';
import { logger } from '@/lib/logger';

const WASM_BASE = 'https://cdn.jsdelivr.net/npm/@mediapipe/tasks-vision@0.10.35/wasm';
const MODEL_URL =
  'https://storage.googleapis.com/mediapipe-models/image_segmenter/selfie_segmenter/float16/latest/selfie_segmenter.tflite';
const BLUR_PX = 12;
const FRAME_INTERVAL_MS = 40;
const PERSON_THRESHOLD = 0;

function makeCanvas(width: number, height: number): HTMLCanvasElement {
  const canvas = document.createElement('canvas');
  canvas.width = width;
  canvas.height = height;
  return canvas;
}

export class BackgroundProcessor {
  private segmenter: ImageSegmenter | null = null;
  private video: HTMLVideoElement | null = null;
  private canvas: HTMLCanvasElement | null = null;
  private maskCanvas: HTMLCanvasElement | null = null;
  private personCanvas: HTMLCanvasElement | null = null;
  private raf = 0;
  private running = false;
  private lastFrame = 0;

  async start(track: MediaStreamTrack): Promise<MediaStreamTrack> {
    this.stop();
    const settings = track.getSettings();
    const width = settings.width ?? 640;
    const height = settings.height ?? 480;

    const vision = await FilesetResolver.forVisionTasks(WASM_BASE);
    this.segmenter = await ImageSegmenter.createFromOptions(vision, {
      baseOptions: { modelAssetPath: MODEL_URL, delegate: 'GPU' },
      runningMode: 'VIDEO',
      outputCategoryMask: true,
      outputConfidenceMasks: false,
    });

    const video = document.createElement('video');
    video.muted = true;
    video.playsInline = true;
    video.srcObject = new MediaStream([track]);
    await video.play();
    this.video = video;

    this.canvas = makeCanvas(width, height);
    this.maskCanvas = makeCanvas(width, height);
    this.personCanvas = makeCanvas(width, height);

    this.running = true;
    this.loop();

    return this.canvas.captureStream(30).getVideoTracks()[0];
  }

  private loop = (): void => {
    if (!this.running) return;
    const now = performance.now();
    if (this.video && this.segmenter && this.video.readyState >= 2 && now - this.lastFrame >= FRAME_INTERVAL_MS) {
      this.lastFrame = now;
      try {
        this.segmenter.segmentForVideo(this.video, now, (result) => this.composite(result));
      } catch (err) {
        logger.error('BackgroundProcessor', 'segmentForVideo', err);
      }
    }
    this.raf = requestAnimationFrame(this.loop);
  };

  private composite(result: ImageSegmenterResult): void {
    const { video, canvas, maskCanvas, personCanvas } = this;
    const mask = result.categoryMask;
    if (!video || !canvas || !maskCanvas || !personCanvas || !mask) {
      result.close();
      return;
    }
    const width = canvas.width;
    const height = canvas.height;
    const ctx = canvas.getContext('2d');
    const mctx = maskCanvas.getContext('2d');
    const pctx = personCanvas.getContext('2d');
    if (!ctx || !mctx || !pctx) {
      mask.close();
      result.close();
      return;
    }

    ctx.save();
    ctx.filter = `blur(${BLUR_PX}px)`;
    ctx.drawImage(video, 0, 0, width, height);
    ctx.restore();

    const data = mask.getAsUint8Array();
    const maskImage = mctx.createImageData(width, height);
    for (let i = 0; i < data.length; i++) {
      const alpha = data[i] > PERSON_THRESHOLD ? 255 : 0;
      const j = i * 4;
      maskImage.data[j] = 255;
      maskImage.data[j + 1] = 255;
      maskImage.data[j + 2] = 255;
      maskImage.data[j + 3] = alpha;
    }
    mctx.putImageData(maskImage, 0, 0);

    pctx.clearRect(0, 0, width, height);
    pctx.globalCompositeOperation = 'source-over';
    pctx.drawImage(video, 0, 0, width, height);
    pctx.globalCompositeOperation = 'destination-in';
    pctx.drawImage(maskCanvas, 0, 0);
    pctx.globalCompositeOperation = 'source-over';

    ctx.drawImage(personCanvas, 0, 0);

    mask.close();
    result.close();
  }

  stop(): void {
    this.running = false;
    if (this.raf) cancelAnimationFrame(this.raf);
    this.raf = 0;
    this.segmenter?.close();
    this.segmenter = null;
    if (this.video) {
      this.video.srcObject = null;
      this.video = null;
    }
    this.canvas = null;
    this.maskCanvas = null;
    this.personCanvas = null;
  }
}
