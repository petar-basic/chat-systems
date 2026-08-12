import MarkdownIt from 'markdown-it';
import type { Token } from 'markdown-it';

const ALLOWED_PROTOCOLS = new Set(['http:', 'https:', 'mailto:']);

/**
 * Built from the `zero` preset and opened up to exactly the constructs the
 * composer can serialise, rather than from the default preset with the rest
 * switched off. A parser that cannot represent headings, tables, images, raw
 * HTML or reference links has no surface there to get wrong.
 *
 * Verified against `createEditorExtensions()` output: bold, italic, strike,
 * inline code, fenced code, blockquote, bullet and ordered lists, links, rules
 * and hard breaks. Underline is absent because tiptap-markdown drops the mark on
 * serialisation — it never reaches storage in the first place.
 */
const md = new MarkdownIt('zero', { html: false, linkify: true, breaks: false }).enable([
  'backticks',
  'blockquote',
  'emphasis',
  'entity',
  'escape',
  'fence',
  'hr',
  'link',
  'linkify',
  'list',
  'newline',
  'strikethrough',
]);

function isAllowedLink(url: string): boolean {
  try {
    return ALLOWED_PROTOCOLS.has(new URL(url, 'https://relative.invalid').protocol);
  } catch {
    return false;
  }
}

md.validateLink = isAllowedLink;

export type MessageNode =
  | { kind: 'text'; text: string }
  | { kind: 'break' }
  | { kind: 'rule' }
  | { kind: 'code'; text: string }
  | { kind: 'codeBlock'; text: string; language: string | null }
  | { kind: 'fragment'; children: MessageNode[] }
  | { kind: 'link'; href: string; children: MessageNode[] }
  | { kind: 'strong'; children: MessageNode[] }
  | { kind: 'em'; children: MessageNode[] }
  | { kind: 'strike'; children: MessageNode[] }
  | { kind: 'paragraph'; children: MessageNode[] }
  | { kind: 'blockquote'; children: MessageNode[] }
  | { kind: 'bulletList'; children: MessageNode[] }
  | { kind: 'orderedList'; start: number; children: MessageNode[] }
  | { kind: 'listItem'; children: MessageNode[] };

type Branch = Extract<MessageNode, { children: MessageNode[] }>;

const INLINE_MARK: Record<string, Branch['kind']> = {
  strong_open: 'strong',
  em_open: 'em',
  s_open: 'strike',
};

const BLOCK_CONTAINER: Record<string, Branch['kind']> = {
  paragraph_open: 'paragraph',
  blockquote_open: 'blockquote',
  bullet_list_open: 'bulletList',
  list_item_open: 'listItem',
};

function branch(kind: Branch['kind'], start = 1): Branch {
  return kind === 'orderedList'
    ? { kind: 'orderedList', start, children: [] }
    : ({ kind, children: [] } as Branch);
}

class TreeBuilder {
  readonly root: MessageNode[] = [];
  private readonly stack: MessageNode[][] = [this.root];

  push(node: MessageNode) {
    this.stack[this.stack.length - 1].push(node);
  }

  open(node: Branch) {
    this.push(node);
    this.stack.push(node.children);
  }

  close() {
    if (this.stack.length > 1) this.stack.pop();
  }
}

function languageOf(token: Token): string | null {
  const info = token.info.trim().split(/\s+/)[0];
  return info || null;
}

function buildInline(tokens: Token[]): MessageNode[] {
  const tree = new TreeBuilder();

  for (const token of tokens) {
    switch (token.type) {
      case 'text':
        if (token.content) tree.push({ kind: 'text', text: token.content });
        break;
      case 'code_inline':
        tree.push({ kind: 'code', text: token.content });
        break;
      case 'softbreak':
        tree.push({ kind: 'text', text: ' ' });
        break;
      case 'hardbreak':
        tree.push({ kind: 'break' });
        break;
      case 'link_open': {
        const href = String(token.attrGet('href') ?? '');
        // A protocol outside the allowlist does not become an anchor with a
        // defused href — it does not become an anchor at all. The label survives
        // as plain text, so nothing is hidden from the reader and nothing is
        // clickable.
        if (href && isAllowedLink(href)) tree.open({ kind: 'link', href, children: [] });
        else tree.open({ kind: 'fragment', children: [] });
        break;
      }
      default: {
        const mark = INLINE_MARK[token.type];
        if (mark) tree.open(branch(mark));
        else if (token.type.endsWith('_close')) tree.close();
      }
    }
  }

  return tree.root;
}

export function parseMessage(content: string): MessageNode[] {
  const tree = new TreeBuilder();

  for (const token of md.parse(content, {})) {
    if (token.type === 'inline') {
      for (const node of buildInline(token.children ?? [])) tree.push(node);
      continue;
    }
    if (token.type === 'fence' || token.type === 'code_block') {
      tree.push({
        kind: 'codeBlock',
        text: token.content.replace(/\n$/, ''),
        language: languageOf(token),
      });
      continue;
    }
    if (token.type === 'hr') {
      tree.push({ kind: 'rule' });
      continue;
    }

    if (token.type === 'ordered_list_open') {
      tree.open(branch('orderedList', Number(token.attrGet('start') ?? 1)));
      continue;
    }
    const container = BLOCK_CONTAINER[token.type];
    if (container) {
      tree.open(branch(container));
      continue;
    }
    if (token.type.endsWith('_close')) tree.close();
  }

  return tree.root;
}
