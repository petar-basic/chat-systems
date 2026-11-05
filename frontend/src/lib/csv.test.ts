import { describe, it, expect } from 'vitest';
import { toCsv } from './csv';

describe('toCsv', () => {
  it('writes one line per row, with numbers and empty cells as they are', () => {
    expect(
      toCsv([
        ['Member', 'Messages', 'Last active'],
        ['Ana', 12, null],
      ]),
    ).toBe('Member,Messages,Last active\r\nAna,12,\r\n');
  });

  it('quotes a cell with a comma, a quote or a line break', () => {
    expect(toCsv([['Doe, Jane', 'say "hi"', 'two\nlines']])).toBe(
      '"Doe, Jane","say ""hi""","two\nlines"\r\n',
    );
  });

  it('keeps a name that looks like a formula as text', () => {
    expect(toCsv([['=HYPERLINK("x")', '+1', '@me', -3]])).toBe(`"'=HYPERLINK(""x"")",'+1,'@me,-3\r\n`);
  });
});
