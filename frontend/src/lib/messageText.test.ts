import { describe, it, expect } from 'vitest';
import { messageText } from './messageText';

describe('messageText', () => {
  it('drops formatting marks and keeps the words', () => {
    expect(messageText('**ship** it _today_ ~~maybe~~ `now`')).toBe('ship it today maybe now');
  });

  it('writes a mention the way it reads', () => {
    expect(messageText('thanks @[Ana Horvat](u-1), see #general')).toBe('thanks @Ana Horvat, see #general');
  });

  it('keeps the label of a link', () => {
    expect(messageText('read [the plan](https://example.com/plan) first')).toBe('read the plan first');
  });

  it('puts each paragraph and code block on its own line', () => {
    expect(messageText('first\n\nsecond\n\n```ts\nconst a = 1;\nconst b = 2;\n```')).toBe(
      'first\nsecond\nconst a = 1;\nconst b = 2;',
    );
  });

  it('keeps hard breaks', () => {
    expect(messageText('line one\\\nline two')).toBe('line one\nline two');
  });

  it('marks list items as they are shown', () => {
    expect(messageText('- milk\n- eggs')).toBe('• milk\n• eggs');
    expect(messageText('3. three\n4. four')).toBe('3. three\n4. four');
  });

  it('keeps a quote without its marker', () => {
    expect(messageText('> said so\n\nagreed')).toBe('said so\nagreed');
  });
});
