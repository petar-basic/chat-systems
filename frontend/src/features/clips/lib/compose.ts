import { tick } from './ticker';

const FPS = 30;
const MAX_WIDTH = 1920;
const BUBBLE_SHARE = 0.24;
const BUBBLE_MARGIN_SHARE = 0.03;

export interface Composed {
  stream: MediaStream;
  stop: () => void;
}

function playing(stream: MediaStream): HTMLVideoElement {
  const video = document.createElement('video');
  video.muted = true;
  video.playsInline = true;
  video.srcObject = stream;
  void video.play().catch(() => undefined);
  return video;
}

export function withBubble(screen: MediaStream, camera: MediaStream): Composed {
  const settings = screen.getVideoTracks()[0]?.getSettings() ?? {};
  const scale = Math.min(1, MAX_WIDTH / (settings.width ?? MAX_WIDTH));
  const width = Math.round((settings.width ?? 1280) * scale);
  const height = Math.round((settings.height ?? 720) * scale);
  const canvas = document.createElement('canvas');
  canvas.width = width;
  canvas.height = height;
  const ctx = canvas.getContext('2d');
  const screenVideo = playing(screen);
  const cameraVideo = playing(camera);
  const size = Math.round(Math.min(width, height) * BUBBLE_SHARE);
  const margin = Math.round(Math.min(width, height) * BUBBLE_MARGIN_SHARE);
  const centre = { x: margin + size / 2, y: height - margin - size / 2 };

  const draw = () => {
    if (!ctx) return;
    if (screenVideo.readyState >= 2) ctx.drawImage(screenVideo, 0, 0, width, height);
    if (cameraVideo.readyState < 2) return;
    const side = Math.min(cameraVideo.videoWidth, cameraVideo.videoHeight);
    const sx = (cameraVideo.videoWidth - side) / 2;
    const sy = (cameraVideo.videoHeight - side) / 2;
    ctx.save();
    ctx.beginPath();
    ctx.arc(centre.x, centre.y, size / 2, 0, Math.PI * 2);
    ctx.clip();
    ctx.drawImage(cameraVideo, sx, sy, side, side, centre.x - size / 2, centre.y - size / 2, size, size);
    ctx.restore();
    ctx.lineWidth = Math.max(2, size / 40);
    ctx.strokeStyle = '#ffffff';
    ctx.beginPath();
    ctx.arc(centre.x, centre.y, size / 2, 0, Math.PI * 2);
    ctx.stroke();
  };

  const stopTicking = tick(1000 / FPS, draw);
  const stream = canvas.captureStream(FPS);
  return {
    stream,
    stop: () => {
      stopTicking();
      stream.getTracks().forEach((track) => track.stop());
      screenVideo.srcObject = null;
      cameraVideo.srcObject = null;
    },
  };
}
