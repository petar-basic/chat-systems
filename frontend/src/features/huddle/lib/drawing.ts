import { avatarIndexFor } from '@/lib/userHelpers';

export type DrawPoint = [number, number];

export const DRAW_BATCH_MS = 50;
export const DRAW_MAX_POINTS = 64;
export const DRAW_FADE_AFTER_MS = 3000;
export const DRAW_FADE_MS = 500;
export const DRAW_STALE_MS = 10_000;

const STROKE_COLORS = [
  '#a855f7',
  '#3b82f6',
  '#22c55e',
  '#f59e0b',
  '#ec4899',
  '#14b8a6',
  '#6366f1',
  '#f43f5e',
];

export function strokeColorFor(userId: string): string {
  return STROKE_COLORS[avatarIndexFor(userId) % STROKE_COLORS.length];
}

export interface Rect {
  x: number;
  y: number;
  width: number;
  height: number;
}

export function contentRect(
  boxWidth: number,
  boxHeight: number,
  videoWidth: number,
  videoHeight: number,
): Rect {
  if (videoWidth <= 0 || videoHeight <= 0) return { x: 0, y: 0, width: boxWidth, height: boxHeight };
  if (boxWidth * videoHeight > boxHeight * videoWidth) {
    const width = (boxHeight * videoWidth) / videoHeight;
    return { x: (boxWidth - width) / 2, y: 0, width, height: boxHeight };
  }
  const height = (boxWidth * videoHeight) / videoWidth;
  return { x: 0, y: (boxHeight - height) / 2, width: boxWidth, height };
}

export function insideContent(rect: Rect, x: number, y: number): boolean {
  return x >= rect.x && x <= rect.x + rect.width && y >= rect.y && y <= rect.y + rect.height;
}

const unit = (n: number) => Math.round(Math.min(1, Math.max(0, n)) * 10_000) / 10_000;

export function toVideo(rect: Rect, x: number, y: number): DrawPoint {
  return [unit((x - rect.x) / rect.width), unit((y - rect.y) / rect.height)];
}

export function toBox(rect: Rect, [u, v]: DrawPoint): [number, number] {
  return [rect.x + u * rect.width, rect.y + v * rect.height];
}

export interface DrawBatch {
  stroke: number;
  points: DrawPoint[];
  done: boolean;
}

export class StrokeBatcher {
  private stroke = Math.floor(Math.random() * 1_000_000_000);
  private pending: DrawPoint[] = [];
  private timer: ReturnType<typeof setTimeout> | null = null;
  private drawing = false;

  constructor(private readonly send: (batch: DrawBatch) => void) {}

  begin(point: DrawPoint): number {
    this.end();
    this.stroke += 1;
    this.drawing = true;
    this.add(point);
    return this.stroke;
  }

  add(point: DrawPoint): void {
    if (!this.drawing) return;
    this.pending.push(point);
    if (this.pending.length >= DRAW_MAX_POINTS) this.flush(false);
    else this.timer ??= setTimeout(() => this.flush(false), DRAW_BATCH_MS);
  }

  end(): void {
    if (!this.drawing) return;
    this.drawing = false;
    this.flush(true);
  }

  private flush(done: boolean): void {
    if (this.timer !== null) clearTimeout(this.timer);
    this.timer = null;
    if (this.pending.length === 0 && !done) return;
    const points = this.pending;
    this.pending = [];
    this.send({ stroke: this.stroke, points, done });
  }
}

export interface BoardStroke {
  key: string;
  userId: string;
  points: DrawPoint[];
  done: boolean;
  lastAt: number;
}

export function fadeStartOf(stroke: BoardStroke): number {
  return stroke.lastAt + (stroke.done ? DRAW_FADE_AFTER_MS : DRAW_STALE_MS);
}

export function strokeAlpha(stroke: BoardStroke, now: number): number {
  const into = now - fadeStartOf(stroke);
  return into <= 0 ? 1 : Math.max(0, 1 - into / DRAW_FADE_MS);
}

type BoardListener = (sharerId: string, stroke: BoardStroke | null) => void;

export class DrawBoard {
  private boards = new Map<string, Map<string, BoardStroke>>();
  private listeners = new Set<BoardListener>();

  apply(
    sharerId: string,
    userId: string,
    stroke: number,
    points: DrawPoint[],
    done: boolean,
    now = Date.now(),
  ): BoardStroke {
    const board = this.boards.get(sharerId) ?? new Map<string, BoardStroke>();
    this.boards.set(sharerId, board);
    const key = `${userId}:${stroke}`;
    const known = board.get(key);
    const next: BoardStroke = known
      ? { ...known, points: [...known.points, ...points], done: known.done || done, lastAt: now }
      : { key, userId, points: [...points], done, lastAt: now };
    board.set(key, next);
    this.emit(sharerId, next);
    return next;
  }

  visible(sharerId: string, now = Date.now()): { stroke: BoardStroke; alpha: number }[] {
    const board = this.boards.get(sharerId);
    if (!board) return [];
    const shown: { stroke: BoardStroke; alpha: number }[] = [];
    for (const [key, stroke] of board) {
      const alpha = strokeAlpha(stroke, now);
      if (alpha > 0) shown.push({ stroke, alpha });
      else board.delete(key);
    }
    if (board.size === 0) this.boards.delete(sharerId);
    return shown;
  }

  clear(sharerId?: string): void {
    const cleared = sharerId === undefined ? [...this.boards.keys()] : [sharerId];
    for (const id of cleared) {
      if (this.boards.delete(id)) this.emit(id, null);
    }
  }

  subscribe(listener: BoardListener): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  private emit(sharerId: string, stroke: BoardStroke | null): void {
    for (const listener of this.listeners) listener(sharerId, stroke);
  }
}

export const drawBoard = new DrawBoard();
