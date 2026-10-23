const MARKS = /\p{M}/gu;

export function fold(text: string): string {
  return text.normalize('NFD').replace(MARKS, '').toLowerCase();
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

export function fragmentRange(text: string, fragment: string): [number, number] | null {
  const wanted = fold(fragment.trim());
  if (!wanted) return null;
  const { folded, offsets } = foldWithOffsets(text);
  const idx = folded.indexOf(wanted);
  return idx === -1 ? null : [offsets[idx], offsets[idx + wanted.length]];
}
