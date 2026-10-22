export interface ForwardedSource {
  content: string;
  authorName: string;
  origin: string;
}

// A forward is an ordinary message that quotes another one: the server has no
// notion of a forward, so the quote has to survive as text.
export function forwardedBody(source: ForwardedSource, comment: string): string {
  const header = `> **${source.authorName}** in ${source.origin}:`;
  const quote = source.content.trim()
    ? `${header}\n${source.content
        .split('\n')
        .map((line) => `> ${line}`)
        .join('\n')}`
    : header;
  const trimmed = comment.trim();
  return trimmed ? `${trimmed}\n\n${quote}` : quote;
}
