import { describe, it, expect } from 'vitest';
import { FILE_KINDS, fileKind, kindOf, type FileKindName } from './fileKinds';
import cases from './fileKindCases.json';

describe('kindOf', () => {
  it.each(cases)('$name ($mime) is $kind, as the server says', ({ mime, name, kind }) => {
    expect(kindOf(mime, name)).toBe(kind);
  });

  it('draws the icon of the kind it finds', () => {
    expect(fileKind('application/octet-stream', 'Q3 budget.xlsx')).toBe(FILE_KINDS.spreadsheet);
    expect(fileKind('application/octet-stream', 'README').label).toBe('File');
  });

  it('has a style for every kind the server returns', () => {
    const kinds: FileKindName[] = cases.map((c) => c.kind as FileKindName);
    expect(new Set(kinds)).toEqual(new Set(Object.keys(FILE_KINDS)));
  });
});
