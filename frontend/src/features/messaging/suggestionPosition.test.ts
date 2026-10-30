import { describe, it, expect, beforeEach } from 'vitest';
import { positionSuggestion } from './suggestionPosition';

const caret = (left: number, top: number) => new DOMRect(left, top, 1, 16);

function popup(width: number) {
  const el = document.createElement('div');
  Object.defineProperty(el, 'offsetWidth', { value: width });
  return el;
}

describe('positionSuggestion', () => {
  beforeEach(() => {
    Object.defineProperty(window, 'innerWidth', { value: 800, configurable: true });
    Object.defineProperty(window, 'innerHeight', { value: 600, configurable: true });
  });

  it('opens at the caret when it fits', () => {
    const el = popup(240);
    positionSuggestion(el, caret(100, 100), 200);

    expect(el.style.left).toBe('100px');
    expect(el.style.top).toBe('120px');
  });

  it('moves left so it ends inside the window near the right edge', () => {
    const el = popup(240);
    positionSuggestion(el, caret(700, 100), 200);

    expect(el.style.left).toBe('552px');
  });

  it('opens above the caret when there is too little room below', () => {
    const el = popup(240);
    positionSuggestion(el, caret(100, 500), 200);

    expect(el.style.top).toBe('');
    expect(el.style.bottom).toBe('104px');
  });

  it('never starts left of the window on a narrow screen', () => {
    Object.defineProperty(window, 'innerWidth', { value: 200, configurable: true });
    const el = popup(240);
    positionSuggestion(el, caret(150, 100), 200);

    expect(el.style.left).toBe('8px');
  });
});
