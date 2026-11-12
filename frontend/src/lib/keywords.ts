import { UNSPACED, fold, foldWithOffsets } from './fold';

const WORD = /[\p{L}\p{N}_]/u;

const separates = (word: string) => !WORD.test(word) || UNSPACED.test(word);

export const KEYWORD_LIMIT = 50;

export function foldKeyword(keyword: string): string {
  return fold(keyword.trim());
}

export type KeywordProblem =
  | { kind: 'length'; min: number; max: number }
  | { kind: 'prefix' }
  | { kind: 'duplicate'; keyword: string }
  | { kind: 'limit'; limit: number };

export function keywordProblem(keyword: string, listed: string[]): KeywordProblem | null {
  const word = keyword.trim();
  const length = [...word].length;
  if (length < 2 || length > 64) return { kind: 'length', min: 2, max: 64 };
  if (word.startsWith('@') || word.startsWith('#')) return { kind: 'prefix' };
  const folded = foldKeyword(word);
  if (listed.some((k) => foldKeyword(k) === folded)) return { kind: 'duplicate', keyword: word };
  if (listed.length >= KEYWORD_LIMIT) return { kind: 'limit', limit: KEYWORD_LIMIT };
  return null;
}

export function keywordRanges(text: string, keywords: string[]): Array<[number, number]> {
  if (keywords.length === 0) return [];
  const { folded, offsets } = foldWithOffsets(text);

  const ranges: Array<[number, number]> = [];
  for (const keyword of keywords) {
    const letters = [...keyword];
    const openStart = UNSPACED.test(letters[0] ?? '');
    const openEnd = UNSPACED.test(letters[letters.length - 1] ?? '');
    let idx = folded.indexOf(keyword);
    while (idx !== -1) {
      const end = idx + keyword.length;
      const before = [...folded.slice(Math.max(0, idx - 2), idx)].pop() ?? '';
      const after = [...folded.slice(end, end + 2)][0] ?? '';
      if ((openStart || separates(before)) && (openEnd || separates(after)))
        ranges.push([offsets[idx], offsets[end]]);
      idx = folded.indexOf(keyword, idx + 1);
    }
  }
  return ranges;
}
