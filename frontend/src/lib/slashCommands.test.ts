import { describe, it, expect } from 'vitest';
import { parseCommand } from './slashCommands';

describe('parseCommand', () => {
  it('splits the name from the rest of what was typed', () => {
    expect(parseCommand('/deploy prod now')).toEqual({ command: 'deploy', text: 'prod now' });
    expect(parseCommand('/dnd')).toEqual({ command: 'dnd', text: '' });
    expect(parseCommand('  /Topic Release week ')).toEqual({
      command: 'topic',
      text: 'Release week',
    });
  });

  /// Everything that is not a command has to reach the channel as text, or a
  /// message that happens to start with a slash disappears.
  it('leaves ordinary messages alone', () => {
    expect(parseCommand('hello')).toBeNull();
    expect(parseCommand('')).toBeNull();
    expect(parseCommand('/')).toBeNull();
    expect(parseCommand('/ leading space')).toBeNull();
    expect(parseCommand('and/or')).toBeNull();
    expect(parseCommand('http://example.com/path')).toBeNull();
  });

  it('keeps a multi-line argument intact', () => {
    expect(parseCommand('/topic first\nsecond')).toEqual({
      command: 'topic',
      text: 'first\nsecond',
    });
  });
});
