import { describe, it, expect, vi, beforeAll } from 'vitest';
import { render, screen, act } from '@testing-library/react';
import VirtualMessageList, { type VirtualRow } from './VirtualMessageList';

const ROW_HEIGHT = 40;
const VIEWPORT_HEIGHT = 400;

/** The scroll container's content height, which jsdom will not compute. */
let totalHeight = VIEWPORT_HEIGHT;
const setTotalHeight = (rowCount: number) => {
  totalHeight = Math.max(VIEWPORT_HEIGHT, rowCount * ROW_HEIGHT);
};

interface Row extends VirtualRow {
  label: string;
}

const row = (label: string): Row => ({ key: label, label, estimatedHeight: ROW_HEIGHT });

/**
 * jsdom reports every element as 0×0, so the virtualizer would window nothing.
 * Heights are stubbed to a fixed row size, which is enough for the three
 * behaviours under test — they are all about scroll position, not layout.
 */
beforeAll(() => {
  Object.defineProperty(HTMLElement.prototype, 'clientHeight', {
    configurable: true,
    get(this: HTMLElement) {
      return this.dataset.index === undefined ? VIEWPORT_HEIGHT : ROW_HEIGHT;
    },
  });
  Object.defineProperty(HTMLElement.prototype, 'scrollHeight', {
    configurable: true,
    get(this: HTMLElement) {
      return this.dataset.qa === 'list' ? totalHeight : VIEWPORT_HEIGHT;
    },
  });
  Object.defineProperty(HTMLElement.prototype, 'getBoundingClientRect', {
    configurable: true,
    value(this: HTMLElement) {
      const height = this.dataset.index === undefined ? VIEWPORT_HEIGHT : ROW_HEIGHT;
      return { width: 800, height, top: 0, left: 0, right: 800, bottom: height, x: 0, y: 0 };
    },
  });
});

function renderList(rows: Row[], overrides: Partial<Parameters<typeof VirtualMessageList>[0]> = {}) {
  const onLoadOlder = vi.fn();
  setTotalHeight(rows.length);
  const utils = render(
    <VirtualMessageList
      rows={rows}
      renderRow={(r) => <div>{(r as Row).label}</div>}
      hasOlder
      isLoadingOlder={false}
      onLoadOlder={onLoadOlder}
      qa="list"
      {...overrides}
    />,
  );
  const scroller = utils.container.querySelector('[data-qa="list"]') as HTMLElement;
  return { ...utils, scroller, onLoadOlder, setTotalHeight };
}

/**
 * A real browser clamps scrollTop to `scrollHeight - clientHeight`; the jsdom
 * stub does not, so "at the bottom" is expressed as a predicate rather than as
 * an equality that only holds because nothing clamps.
 */
function isAtBottom(scroller: HTMLElement): boolean {
  return scroller.scrollHeight - scroller.scrollTop - scroller.clientHeight <= 1;
}

function scrollTo(scroller: HTMLElement, top: number) {
  Object.defineProperty(scroller, 'scrollTop', { configurable: true, writable: true, value: top });
  act(() => {
    scroller.dispatchEvent(new Event('scroll'));
  });
}

describe('VirtualMessageList scroll anchoring', () => {
  it('starts at the bottom', () => {
    const rows = Array.from({ length: 40 }, (_, i) => row(`m${i}`));
    const { scroller } = renderList(rows);
    expect(isAtBottom(scroller)).toBe(true);
  });

  it('asks for the previous page when scrolled near the top', () => {
    const rows = Array.from({ length: 40 }, (_, i) => row(`m${i}`));
    const { scroller, onLoadOlder } = renderList(rows);

    scrollTo(scroller, 50);
    expect(onLoadOlder).toHaveBeenCalled();
  });

  // Holding the viewport still when a page lands is anchored on the real
  // position of a real row, which jsdom does not compute — every element reports
  // a zero-sized rect. That behaviour is covered end to end instead, in
  // `e2e/message-pagination.spec.ts`, where there is a browser to measure.

  it('sticks to the bottom for a new message when already at the bottom', () => {
    const rows = Array.from({ length: 40 }, (_, i) => row(`m${i}`));
    const { scroller, rerender, setTotalHeight } = renderList(rows);

    scrollTo(scroller, 40 * ROW_HEIGHT);
    setTotalHeight(41);
    rerender(
      <VirtualMessageList
        rows={[...rows, row('fresh')]}
        renderRow={(r) => <div>{(r as Row).label}</div>}
        hasOlder
        isLoadingOlder={false}
        onLoadOlder={vi.fn()}
        qa="list"
      />,
    );

    expect(isAtBottom(scroller)).toBe(true);
    expect(screen.queryByTestId('jump-to-latest')).toBeNull();
  });

  it('offers a jump instead of yanking the viewport when scrolled up', () => {
    const rows = Array.from({ length: 40 }, (_, i) => row(`m${i}`));
    const { scroller, rerender, setTotalHeight } = renderList(rows);

    scrollTo(scroller, 400);
    const before = scroller.scrollTop;

    setTotalHeight(41);
    rerender(
      <VirtualMessageList
        rows={[...rows, row('fresh')]}
        renderRow={(r) => <div>{(r as Row).label}</div>}
        hasOlder
        isLoadingOlder={false}
        onLoadOlder={vi.fn()}
        qa="list"
      />,
    );

    expect(scroller.scrollTop).toBe(before);
    expect(screen.getByTestId('jump-to-latest')).toBeInTheDocument();
  });
});
