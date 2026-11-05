import { fold, foldWithOffsets } from './fold';

const WORD = /[\p{L}\p{N}_]/u;

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
    let idx = folded.indexOf(keyword);
    while (idx !== -1) {
      const end = idx + keyword.length;
      const before = idx === 0 ? '' : folded[idx - 1];
      const after = end < folded.length ? folded[end] : '';
      if (!WORD.test(before) && !WORD.test(after)) ranges.push([offsets[idx], offsets[end]]);
      idx = folded.indexOf(keyword, idx + 1);
    }
  }
  return ranges;
}
