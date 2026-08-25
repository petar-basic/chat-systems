export interface ForwardedSource {
  content: string;
  authorName: string;
  origin: string;
}

// A forward is an ordinary message that quotes another one: the server has no
// notion of a forward, so the quote has to survive as text.
export function forwardedBody(source: ForwardedSource, comment: string): string {
  const quoted = source.content
    .split('\n')
    .map((line) => `> ${line}`)
    .join('\n');
  const header = `> **${source.authorName}** in ${source.origin}:`;
  const trimmed = comment.trim();
  return trimmed ? `${trimmed}\n\n${header}\n${quoted}` : `${header}\n${quoted}`;
}
