import { describe, it, expect } from 'vitest';
import { forwardedBody } from './forwardBody';

const source = { content: 'the deploy is stuck', authorName: 'Ana', origin: '#ops' };

describe('forwardedBody', () => {
  it('quotes the original under the comment', () => {
    expect(forwardedBody(source, 'seen this?')).toBe(
      'seen this?\n\n> **Ana** in #ops:\n> the deploy is stuck',
    );
  });

  it('is just the quote when there is nothing to add', () => {
    expect(forwardedBody(source, '   ')).toBe('> **Ana** in #ops:\n> the deploy is stuck');
  });

  it('keeps a multi-line message inside the quote', () => {
    expect(forwardedBody({ ...source, content: 'one\ntwo' }, '')).toBe('> **Ana** in #ops:\n> one\n> two');
  });
});
