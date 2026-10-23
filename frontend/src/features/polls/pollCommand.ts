export const MIN_OPTIONS = 2;
export const MAX_OPTIONS = 10;
export const QUESTION_MAX = 300;
export const OPTION_MAX = 100;

export type PollCommand =
  | { kind: 'dialog' }
  | { kind: 'create'; question: string; options: string[] }
  | { kind: 'invalid'; message: string };

const USAGE = 'Try /poll "Lunch?" "Thursday" "Friday", or /poll alone for the dialog';
const QUOTES: Record<string, string> = { '"': '"', '“': '”', '„': '“', '”': '”' };

export function parsePollCommand(content: string): PollCommand | null {
  const match = /^\s*\/poll(?:\s+([\s\S]*))?$/i.exec(content);
  if (!match) return null;
  const rest = (match[1] ?? '').trim();
  if (!rest) return { kind: 'dialog' };

  const parts: string[] = [];
  let i = 0;
  while (i < rest.length) {
    const ch = rest[i];
    if (/\s/.test(ch)) {
      i += 1;
      continue;
    }
    const close = QUOTES[ch];
    if (!close) return { kind: 'invalid', message: USAGE };
    const end = rest.indexOf(close, i + 1);
    if (end < 0) return { kind: 'invalid', message: 'A quote is not closed' };
    parts.push(rest.slice(i + 1, end).trim());
    i = end + 1;
  }

  const [question, ...options] = parts;
  if (!question) return { kind: 'invalid', message: USAGE };
  const kept = options.filter(Boolean);
  if (kept.length < MIN_OPTIONS || kept.length > MAX_OPTIONS) {
    return { kind: 'invalid', message: `A poll needs ${MIN_OPTIONS} to ${MAX_OPTIONS} options` };
  }
  return { kind: 'create', question, options: kept };
}

export function moveRow<T>(rows: T[], from: number, to: number): T[] {
  if (from === to || to < 0 || to >= rows.length) return rows;
  const next = [...rows];
  const [moved] = next.splice(from, 1);
  next.splice(to, 0, moved);
  return next;
}
