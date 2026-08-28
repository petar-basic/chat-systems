const FENCED_CODE = /```[\s\S]*?```/g;
const INLINE_CODE = /`([^`]*)`/g;
const IMAGE = /!\[([^\]]*)\]\([^)]*\)/g;
const LINK = /\[([^\]]*)\]\([^)]*\)/g;
const EMPHASIS = /(\*\*\*|\*\*|\*|___|__|_|~~)(.*?)\1/g;
const HEADING = /^\s{0,3}#{1,6}\s+/gm;
const QUOTE = /^\s{0,3}>\s?/gm;
const BULLET = /^\s{0,3}[-*+]\s+/gm;
const RULE = /^\s{0,3}([-*_])\s*(?:\1\s*){2,}$/gm;

export function toPlainText(markdown: string): string {
  return markdown
    .replace(FENCED_CODE, ' ')
    .replace(IMAGE, '$1')
    .replace(LINK, '$1')
    .replace(INLINE_CODE, '$1')
    .replace(EMPHASIS, '$2')
    .replace(HEADING, '')
    .replace(QUOTE, '')
    .replace(RULE, ' ')
    .replace(BULLET, '')
    .replace(/\s+/g, ' ')
    .trim();
}
