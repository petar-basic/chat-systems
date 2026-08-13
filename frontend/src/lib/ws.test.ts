import { describe, it, expect } from 'vitest';
import { isNewerStreamId } from './ws';

describe('isNewerStreamId', () => {
  it('compares both halves as numbers', () => {
    expect(isNewerStreamId('100-2', '100-1')).toBe(true);
    expect(isNewerStreamId('100-1', '100-2')).toBe(false);
    expect(isNewerStreamId('101-0', '100-9')).toBe(true);
  });

  it('does not compare ids as text', () => {
    // "9" sorts after "1" as a string, so a text comparison would call 9-0 newer.
    expect(isNewerStreamId('9-0', '100-0')).toBe(false);
    expect(isNewerStreamId('100-0', '9-0')).toBe(true);
  });

  it('treats the same position as not newer, so it cannot go backwards', () => {
    expect(isNewerStreamId('100-1', '100-1')).toBe(false);
  });

  it('accepts an unparseable id rather than getting stuck on it', () => {
    expect(isNewerStreamId('100-1', 'nonsense')).toBe(true);
  });
});

describe('duplicate suppression', () => {
  /** The dispatch guard, expressed on its own: apply only what is strictly newer. */
  function applied(ids: string[]): string[] {
    let seen: string | undefined;
    const out: string[] = [];
    for (const id of ids) {
      if (seen && !isNewerStreamId(id, seen)) continue;
      seen = id;
      out.push(id);
    }
    return out;
  }

  it('drops an event that arrives again in a replay', () => {
    expect(applied(['100-0', '100-1', '100-1', '100-2'])).toEqual(['100-0', '100-1', '100-2']);
  });

  it('drops a whole overlapping window, not just exact repeats', () => {
    // Replay hands back 100-1..100-3 after they were already seen live.
    expect(applied(['100-1', '100-2', '100-3', '100-1', '100-2', '100-3', '100-4'])).toEqual([
      '100-1',
      '100-2',
      '100-3',
      '100-4',
    ]);
  });

  it('keeps everything when nothing repeats', () => {
    expect(applied(['9-0', '10-0', '100-0'])).toEqual(['9-0', '10-0', '100-0']);
  });
});
