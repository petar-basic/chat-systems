export interface MentionRef {
  label: string;
  id: string;
}

export interface MentionSpan {
  text: string;
  mention: 'self' | 'other' | null;
}

const BROADCAST_MENTIONS = ['here', 'everyone', 'channel'];

function isStartBoundary(ch: string) {
  return ch === '' || ch === ' ' || ch === '\n' || ch === '(';
}

function isWordChar(ch: string) {
  return /[A-Za-z0-9_]/.test(ch);
}

/**
 * Splits a run of text into plain and highlighted spans. The rules are the ones
 * the ProseMirror decoration plugin used: longest label first so "Ana Marija"
 * wins over "Ana", a boundary check on the left so an email address is not a
 * mention, and no overlapping matches.
 */
export function highlightMentions(
  text: string,
  selfId: string | undefined,
  mentions: MentionRef[],
): MentionSpan[] {
  if (!text.includes('@')) return [{ text, mention: null }];

  const taken: Array<[number, number, 'self' | 'other']> = [];
  const overlaps = (start: number, end: number) => taken.some(([ts, te]) => start < te && end > ts);

  const claim = (start: number, len: number, kind: 'self' | 'other') => {
    const end = start + len;
    if (overlaps(start, end)) return;
    taken.push([start, end, kind]);
  };

  for (const mention of [...mentions].sort((a, b) => b.label.length - a.label.length)) {
    const needle = `@${mention.label}`;
    let idx = text.indexOf(needle);
    while (idx !== -1) {
      if (isStartBoundary(idx === 0 ? '' : text[idx - 1])) {
        claim(idx, needle.length, selfId !== undefined && mention.id === selfId ? 'self' : 'other');
      }
      idx = text.indexOf(needle, idx + needle.length);
    }
  }

  for (const word of BROADCAST_MENTIONS) {
    const needle = `@${word}`;
    let idx = text.indexOf(needle);
    while (idx !== -1) {
      const before = idx === 0 ? '' : text[idx - 1];
      const afterIdx = idx + needle.length;
      const after = afterIdx < text.length ? text[afterIdx] : '';
      if (isStartBoundary(before) && !isWordChar(after)) claim(idx, needle.length, 'self');
      idx = text.indexOf(needle, idx + needle.length);
    }
  }

  if (taken.length === 0) return [{ text, mention: null }];

  taken.sort((a, b) => a[0] - b[0]);
  const spans: MentionSpan[] = [];
  let cursor = 0;
  for (const [start, end, kind] of taken) {
    if (start > cursor) spans.push({ text: text.slice(cursor, start), mention: null });
    spans.push({ text: text.slice(start, end), mention: kind });
    cursor = end;
  }
  if (cursor < text.length) spans.push({ text: text.slice(cursor), mention: null });
  return spans;
}
