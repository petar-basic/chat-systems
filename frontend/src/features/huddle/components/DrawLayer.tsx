import { useCallback, useEffect, useRef, type PointerEvent, type RefObject } from 'react';
import { useHuddleStore } from '@/stores/huddle';
import { useUserCache } from '@/stores/users';
import { displayNameOf } from '@/lib/userHelpers';
import { instanceManager } from '@/lib/instances';
import {
  contentRect,
  drawBoard,
  insideContent,
  StrokeBatcher,
  strokeColorFor,
  toBox,
  toVideo,
  type BoardStroke,
  type DrawBatch,
  type DrawPoint,
  type Rect,
} from '../lib/drawing';

interface Props {
  sharerId: string;
  video: RefObject<HTMLVideoElement | null>;
  pen: boolean;
}

const LINE_WIDTH = 3;
const MIN_STEP = 0.002;

function sendBatch(sharerId: string, batch: DrawBatch) {
  const active = useHuddleStore.getState().active;
  if (!active) return;
  instanceManager.get(active.instanceUrl).ws.send({
    type: 'huddle.draw',
    huddle_id: active.huddleId,
    sharer_id: sharerId,
    ...batch,
  });
}

function paintStroke(
  ctx: CanvasRenderingContext2D,
  rect: Rect,
  stroke: BoardStroke,
  alpha: number,
  name: string,
  boxWidth: number,
) {
  const [first, ...rest] = stroke.points.map((point) => toBox(rect, point));
  if (!first) return;
  const color = strokeColorFor(stroke.userId);
  ctx.globalAlpha = alpha;
  ctx.strokeStyle = color;
  ctx.lineWidth = LINE_WIDTH;
  ctx.lineCap = 'round';
  ctx.lineJoin = 'round';
  ctx.beginPath();
  ctx.moveTo(first[0], first[1]);
  if (rest.length === 0) ctx.lineTo(first[0] + 0.1, first[1]);
  for (const [x, y] of rest) ctx.lineTo(x, y);
  ctx.stroke();

  ctx.font = '600 11px system-ui, sans-serif';
  ctx.textBaseline = 'middle';
  const width = ctx.measureText(name).width + 10;
  const x = Math.max(0, Math.min(first[0] + 6, boxWidth - width));
  const y = Math.max(0, first[1] - 22);
  ctx.fillStyle = color;
  ctx.beginPath();
  ctx.roundRect(x, y, width, 16, 4);
  ctx.fill();
  ctx.fillStyle = '#fff';
  ctx.fillText(name, x + 5, y + 8);
  ctx.globalAlpha = 1;
}

export function DrawLayer({ sharerId, video, pen }: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const batcherRef = useRef<StrokeBatcher | null>(null);
  const strokeRef = useRef<{ id: number; last: DrawPoint } | null>(null);
  const selfId = useHuddleStore((s) => s.active?.selfUserId ?? null);
  const { getUser } = useUserCache();
  const nameOfRef = useRef(getUser);

  useEffect(() => {
    nameOfRef.current = getUser;
  }, [getUser]);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    let frame = 0;
    const paint = () => {
      frame = 0;
      const ctx = canvas.getContext('2d');
      if (!ctx) return;
      const dpr = window.devicePixelRatio || 1;
      const width = canvas.clientWidth;
      const height = canvas.clientHeight;
      if (canvas.width !== Math.round(width * dpr) || canvas.height !== Math.round(height * dpr)) {
        canvas.width = Math.round(width * dpr);
        canvas.height = Math.round(height * dpr);
      }
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      ctx.clearRect(0, 0, width, height);
      const rect = contentRect(
        width,
        height,
        video.current?.videoWidth ?? 0,
        video.current?.videoHeight ?? 0,
      );
      const shown = drawBoard.visible(sharerId);
      for (const { stroke, alpha } of shown) {
        const name = displayNameOf(nameOfRef.current(stroke.userId)?.display_name);
        paintStroke(ctx, rect, stroke, alpha, name, width);
      }
      if (shown.length > 0) frame = requestAnimationFrame(paint);
    };
    const schedule = () => {
      if (!frame) frame = requestAnimationFrame(paint);
    };
    const off = drawBoard.subscribe((id) => {
      if (id === sharerId) schedule();
    });
    const resized = new ResizeObserver(schedule);
    resized.observe(canvas);
    schedule();
    return () => {
      off();
      resized.disconnect();
      if (frame) cancelAnimationFrame(frame);
    };
  }, [sharerId, video]);

  useEffect(() => {
    const batcher = new StrokeBatcher((batch) => sendBatch(sharerId, batch));
    batcherRef.current = batcher;
    return () => {
      batcher.end();
      batcherRef.current = null;
    };
  }, [sharerId]);

  const finish = useCallback(() => {
    const current = strokeRef.current;
    strokeRef.current = null;
    if (!current || !selfId) return;
    batcherRef.current?.end();
    drawBoard.apply(sharerId, selfId, current.id, [], true);
  }, [sharerId, selfId]);

  useEffect(() => {
    if (!pen) finish();
  }, [pen, finish]);

  const rectOf = (canvas: HTMLCanvasElement) =>
    contentRect(
      canvas.clientWidth,
      canvas.clientHeight,
      video.current?.videoWidth ?? 0,
      video.current?.videoHeight ?? 0,
    );

  const pointIn = (e: PointerEvent<HTMLCanvasElement>): [number, number] => {
    const box = e.currentTarget.getBoundingClientRect();
    return [e.clientX - box.left, e.clientY - box.top];
  };

  const begin = (e: PointerEvent<HTMLCanvasElement>) => {
    const batcher = batcherRef.current;
    if (!pen || !selfId || !batcher) return;
    const rect = rectOf(e.currentTarget);
    const [x, y] = pointIn(e);
    if (!insideContent(rect, x, y)) return;
    e.preventDefault();
    e.currentTarget.setPointerCapture(e.pointerId);
    const point = toVideo(rect, x, y);
    const id = batcher.begin(point);
    strokeRef.current = { id, last: point };
    drawBoard.apply(sharerId, selfId, id, [point], false);
  };

  const move = (e: PointerEvent<HTMLCanvasElement>) => {
    const current = strokeRef.current;
    if (!current || !selfId) return;
    const [x, y] = pointIn(e);
    const point = toVideo(rectOf(e.currentTarget), x, y);
    if (Math.hypot(point[0] - current.last[0], point[1] - current.last[1]) < MIN_STEP) return;
    current.last = point;
    batcherRef.current?.add(point);
    drawBoard.apply(sharerId, selfId, current.id, [point], false);
  };

  return (
    <canvas
      ref={canvasRef}
      data-qa="huddle-draw-layer"
      data-pen={pen}
      onPointerDown={begin}
      onPointerMove={move}
      onPointerUp={finish}
      onPointerCancel={finish}
      className={`absolute inset-0 w-full h-full ${pen ? 'cursor-crosshair touch-none' : 'pointer-events-none'}`}
    />
  );
}
