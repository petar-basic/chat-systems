import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import {
  contentRect,
  DrawBoard,
  DRAW_BATCH_MS,
  DRAW_FADE_AFTER_MS,
  DRAW_FADE_MS,
  DRAW_MAX_POINTS,
  DRAW_STALE_MS,
  insideContent,
  StrokeBatcher,
  toBox,
  toVideo,
  type DrawBatch,
  type DrawPoint,
} from './drawing';

describe('mapping a point between a tile and the shared video', () => {
  const video = { width: 1920, height: 1080 };
  const small = contentRect(112, 84, video.width, video.height);
  const large = contentRect(640, 360, video.width, video.height);

  it('fits the video inside each tile, letterboxed where the shapes differ', () => {
    expect(small).toEqual({ x: 0, y: 10.5, width: 112, height: 63 });
    expect(large).toEqual({ x: 0, y: 0, width: 640, height: 360 });
    expect(contentRect(400, 300, 1440, 1080)).toEqual({ x: 0, y: 0, width: 400, height: 300 });
    expect(contentRect(400, 100, 1440, 1080)).toEqual({
      x: (400 - 400 / 3) / 2,
      y: 0,
      width: 400 / 3,
      height: 100,
    });
  });

  it('lands the same spot of the screen on both tile sizes', () => {
    const fromSmall = toVideo(small, 28, 10.5 + 63 / 2);
    expect(fromSmall).toEqual([0.25, 0.5]);
    expect(toBox(large, fromSmall)).toEqual([160, 180]);
    expect(toVideo(large, 160, 180)).toEqual(fromSmall);
  });

  it('keeps the letterbox bars out of the drawing', () => {
    expect(insideContent(small, 56, 5)).toBe(false);
    expect(insideContent(small, 56, 42)).toBe(true);
    expect(toVideo(small, 56, 5)).toEqual([0.5, 0]);
    expect(toVideo(small, 200, 100)).toEqual([1, 1]);
  });

  it('sends four decimals at most', () => {
    expect(toVideo(large, 100, 100)).toEqual([0.1563, 0.2778]);
  });
});

describe('batching a stroke for the wire', () => {
  let sent: DrawBatch[];
  let batcher: StrokeBatcher;

  beforeEach(() => {
    vi.useFakeTimers();
    sent = [];
    batcher = new StrokeBatcher((batch) => sent.push(batch));
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it(`sends the points of each ${DRAW_BATCH_MS} ms together`, () => {
    const stroke = batcher.begin([0.1, 0.1]);
    batcher.add([0.2, 0.2]);
    expect(sent).toEqual([]);
    vi.advanceTimersByTime(DRAW_BATCH_MS);
    expect(sent).toEqual([
      {
        stroke,
        points: [
          [0.1, 0.1],
          [0.2, 0.2],
        ],
        done: false,
      },
    ]);

    batcher.add([0.3, 0.3]);
    vi.advanceTimersByTime(DRAW_BATCH_MS - 1);
    expect(sent).toHaveLength(1);
    vi.advanceTimersByTime(1);
    expect(sent[1]).toEqual({ stroke, points: [[0.3, 0.3]], done: false });
  });

  it(`sends at once when a batch reaches ${DRAW_MAX_POINTS} points`, () => {
    const stroke = batcher.begin([0, 0]);
    for (let i = 1; i < DRAW_MAX_POINTS + 3; i++) batcher.add([i / 100, 0]);
    expect(sent).toHaveLength(1);
    expect(sent[0].points).toHaveLength(DRAW_MAX_POINTS);
    vi.advanceTimersByTime(DRAW_BATCH_MS);
    expect(sent[1]).toEqual({
      stroke,
      points: [
        [0.64, 0],
        [0.65, 0],
        [0.66, 0],
      ],
      done: false,
    });
  });

  it('ends a stroke with what is left, marked done, and nothing after', () => {
    const first = batcher.begin([0.5, 0.5]);
    batcher.end();
    expect(sent).toEqual([{ stroke: first, points: [[0.5, 0.5]], done: true }]);

    const second = batcher.begin([0.6, 0.6]);
    vi.advanceTimersByTime(DRAW_BATCH_MS);
    batcher.end();
    batcher.add([0.7, 0.7]);
    batcher.end();
    vi.advanceTimersByTime(DRAW_BATCH_MS);
    expect(second).toBe(first + 1);
    expect(sent.slice(1)).toEqual([
      { stroke: second, points: [[0.6, 0.6]], done: false },
      { stroke: second, points: [], done: true },
    ]);
  });

  it('closes the open stroke when a new one begins', () => {
    const first = batcher.begin([0.1, 0.1]);
    const second = batcher.begin([0.9, 0.9]);
    expect(sent).toEqual([{ stroke: first, points: [[0.1, 0.1]], done: true }]);
    vi.advanceTimersByTime(DRAW_BATCH_MS);
    expect(sent[1]).toEqual({ stroke: second, points: [[0.9, 0.9]], done: false });
  });
});

describe('fading strokes off the board', () => {
  const line: DrawPoint[] = [
    [0.1, 0.1],
    [0.2, 0.2],
  ];

  it(`keeps a finished stroke for ${DRAW_FADE_AFTER_MS / 1000} s, then fades it out`, () => {
    const board = new DrawBoard();
    board.apply('ana', 'bob', 1, line.slice(0, 1), false, 1_000);
    board.apply('ana', 'bob', 1, line.slice(1), true, 2_000);

    expect(board.visible('ana', 2_000 + DRAW_FADE_AFTER_MS)).toEqual([
      { stroke: { key: 'bob:1', userId: 'bob', points: line, done: true, lastAt: 2_000 }, alpha: 1 },
    ]);
    expect(board.visible('ana', 2_000 + DRAW_FADE_AFTER_MS + DRAW_FADE_MS / 2)[0].alpha).toBe(0.5);
    expect(board.visible('ana', 2_000 + DRAW_FADE_AFTER_MS + DRAW_FADE_MS)).toEqual([]);
    expect(board.visible('ana', 0)).toEqual([]);
  });

  it('lets a stroke whose end never came go stale', () => {
    const board = new DrawBoard();
    board.apply('ana', 'bob', 7, line, false, 0);
    expect(board.visible('ana', DRAW_FADE_AFTER_MS + DRAW_FADE_MS)).toHaveLength(1);
    expect(board.visible('ana', DRAW_STALE_MS + DRAW_FADE_MS)).toEqual([]);
  });

  it("keeps each sharer's board apart and tells who changed", () => {
    const board = new DrawBoard();
    const heard = vi.fn();
    const off = board.subscribe(heard);
    board.apply('ana', 'bob', 1, line, false, 0);
    board.apply('cleo', 'bob', 1, line, false, 0);
    board.clear('ana');
    expect(board.visible('ana', 0)).toEqual([]);
    expect(board.visible('cleo', 0)).toHaveLength(1);
    expect(heard.mock.calls.map(([id, stroke]) => [id, stroke?.key ?? null])).toEqual([
      ['ana', 'bob:1'],
      ['cleo', 'bob:1'],
      ['ana', null],
    ]);
    off();
    board.clear();
    expect(heard).toHaveBeenCalledTimes(3);
    expect(board.visible('cleo', 0)).toEqual([]);
  });
});
