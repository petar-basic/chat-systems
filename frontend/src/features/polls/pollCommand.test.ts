import { describe, expect, it } from 'vitest';
import { parsePollCommand } from './pollCommand';

describe('parsePollCommand', () => {
  it('leaves anything that is not /poll alone', () => {
    expect(parsePollCommand('hello')).toBeNull();
    expect(parsePollCommand('/pollster')).toBeNull();
    expect(parsePollCommand('/remind me in 5m')).toBeNull();
  });

  it('opens the dialog when nothing follows', () => {
    expect(parsePollCommand('/poll')).toEqual({ kind: 'dialog' });
    expect(parsePollCommand('  /POLL   ')).toEqual({ kind: 'dialog' });
  });

  it('reads a quoted question and quoted options', () => {
    expect(parsePollCommand('/poll "Lunch Thursday or Friday?" "Thursday" "Friday, after 1"')).toEqual({
      kind: 'create',
      question: 'Lunch Thursday or Friday?',
      options: ['Thursday', 'Friday, after 1'],
    });
  });

  it('accepts the quotes a phone keyboard types', () => {
    expect(parsePollCommand('/poll “Ručak?” „Četvrtak“ “Petak”')).toEqual({
      kind: 'create',
      question: 'Ručak?',
      options: ['Četvrtak', 'Petak'],
    });
  });

  it('explains what is wrong instead of sending it as text', () => {
    expect(parsePollCommand('/poll Lunch? Thursday Friday')).toMatchObject({ kind: 'invalid' });
    expect(parsePollCommand('/poll "Lunch?" "Thursday')).toEqual({
      kind: 'invalid',
      message: 'A quote is not closed',
    });
    expect(parsePollCommand('/poll "Lunch?" "Thursday"')).toEqual({
      kind: 'invalid',
      message: 'A poll needs 2 to 10 options',
    });
    const eleven = Array.from({ length: 11 }, (_, i) => `"${i}"`).join(' ');
    expect(parsePollCommand(`/poll "Q" ${eleven}`)).toMatchObject({ kind: 'invalid' });
  });
});
