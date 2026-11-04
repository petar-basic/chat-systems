import { describe, it, expect } from 'vitest';
import { enterOutcome } from './enterKey';

const plain = { mod: false, picking: false, inBlock: false };

describe('enterOutcome', () => {
  it('sends on Enter by default, and on ⌘Enter or Ctrl+Enter in either mode', () => {
    expect(enterOutcome('send', plain)).toBe('send');
    expect(enterOutcome('send', { ...plain, mod: true })).toBe('send');
    expect(enterOutcome('newline', { ...plain, mod: true })).toBe('send');
  });

  it('starts a new line on Enter when that is the choice', () => {
    expect(enterOutcome('newline', plain)).toBe('newline');
  });

  it('leaves Enter to a list or a code block in both modes', () => {
    expect(enterOutcome('send', { ...plain, inBlock: true })).toBe('default');
    expect(enterOutcome('newline', { ...plain, inBlock: true })).toBe('default');
    expect(enterOutcome('newline', { ...plain, inBlock: true, mod: true })).toBe('send');
  });

  it('leaves Enter to an open mention, emoji or command picker in both modes', () => {
    for (const mode of ['send', 'newline'] as const) {
      expect(enterOutcome(mode, { ...plain, picking: true })).toBe('default');
      expect(enterOutcome(mode, { ...plain, picking: true, mod: true })).toBe('default');
    }
  });
});
