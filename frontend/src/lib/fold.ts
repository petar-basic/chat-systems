const MARKS = /\p{M}/gu;

export const UNSPACED =
  /[\u0e00-\u0eff\u1000-\u109f\u1780-\u17ff\u3005-\u3007\u3040-\u30ff\u31f0-\u31ff\u3400-\u4dbf\u4e00-\u9fff\uf900-\ufaff\uff66-\uff9f\u{20000}-\u{3134f}]/u;

export function fold(text: string): string {
  return text.normalize('NFKD').replace(MARKS, '').toLowerCase();
}

export function foldWithOffsets(text: string): { folded: string; offsets: number[] } {
  const offsets: number[] = [];
  let folded = '';
  let position = 0;
  for (const ch of text) {
    const part = fold(ch);
    for (let i = 0; i < part.length; i++) offsets.push(position);
    folded += part;
    position += ch.length;
  }
  offsets.push(text.length);
  return { folded, offsets };
}

export function wordRanges(text: string, query: string): [number, number][] {
  const words = new Set(
    fold(query)
      .split(/\s+/)
      .map((word) => word.replace(/^["'(]+|["'),.!?]+$/g, ''))
      .filter((word) => word.length > 1 || UNSPACED.test(word)),
  );
  if (words.size === 0) return [];
  const { folded, offsets } = foldWithOffsets(text);
  const found: [number, number][] = [];
  for (const word of words) {
    for (let at = folded.indexOf(word); at !== -1; at = folded.indexOf(word, at + word.length)) {
      found.push([at, at + word.length]);
    }
  }
  found.sort((a, b) => a[0] - b[0]);
  const merged: [number, number][] = [];
  for (const [start, end] of found) {
    const last = merged[merged.length - 1];
    if (last && start <= last[1]) last[1] = Math.max(last[1], end);
    else merged.push([start, end]);
  }
  return merged.map(([start, end]) => [offsets[start], offsets[end]]);
}
