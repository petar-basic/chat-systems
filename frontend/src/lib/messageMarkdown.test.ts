import { describe, it, expect } from 'vitest';
import { Editor, type Content } from '@tiptap/core';
import { parseMessage, type MessageNode } from './messageMarkdown';
import { createEditorExtensions } from './tiptapExtensions';

/**
 * Every construct the composer can serialise, expressed the way the composer
 * expresses it. `composerOutput` proves the left column is what actually reaches
 * storage, so the renderer is tested against reality rather than against what
 * markdown could theoretically contain.
 */
const p = (...content: unknown[]) => ({ type: 'paragraph', content });
const t = (text: string, marks?: string[]) => ({
  type: 'text',
  text,
  ...(marks ? { marks: marks.map((type) => ({ type })) } : {}),
});
const doc = (...content: unknown[]) => ({ type: 'doc', content }) as Content;

function composerOutput(document: Content): string {
  const editor = new Editor({ extensions: createEditorExtensions(), content: document });
  const markdown = editor.storage.markdown.getMarkdown();
  editor.destroy();
  return markdown;
}

function kinds(nodes: MessageNode[]): string[] {
  return nodes.flatMap((node) => ('children' in node ? [node.kind, ...kinds(node.children)] : [node.kind]));
}

function textOf(nodes: MessageNode[]): string {
  return nodes
    .map((node) => {
      if (node.kind === 'text') return node.text;
      if (node.kind === 'code' || node.kind === 'codeBlock') return node.text;
      if ('children' in node) return textOf(node.children);
      return '';
    })
    .join('');
}

describe('the composer subset survives the round trip', () => {
  const cases: Array<[string, Content, string]> = [
    ['bold', doc(p(t('bold', ['bold']))), '**bold**'],
    ['italic', doc(p(t('it', ['italic']))), '*it*'],
    ['strike', doc(p(t('struck', ['strike']))), '~~struck~~'],
    ['inline code', doc(p(t('x=1', ['code']))), '`x=1`'],
    ['nested marks', doc(p(t('both', ['bold', 'italic']))), '***both***'],
  ];

  for (const [name, document, expected] of cases) {
    it(`serialises ${name} the way the parser expects`, () => {
      expect(composerOutput(document)).toBe(expected);
    });
  }

  it('drops underline before it ever reaches storage', () => {
    expect(composerOutput(doc(p(t('under', ['underline']))))).toBe('under');
  });
});

describe('parseMessage', () => {
  it('renders the inline marks the composer produces', () => {
    expect(kinds(parseMessage('**b** *i* ~~s~~ `c`'))).toEqual([
      'paragraph',
      'strong',
      'text',
      'text',
      'em',
      'text',
      'text',
      'strike',
      'text',
      'text',
      'code',
    ]);
  });

  it('renders blocks', () => {
    expect(kinds(parseMessage('> quoted'))).toEqual(['blockquote', 'paragraph', 'text']);
    expect(kinds(parseMessage('- a\n- b'))).toEqual([
      'bulletList',
      'listItem',
      'paragraph',
      'text',
      'listItem',
      'paragraph',
      'text',
    ]);
    expect(kinds(parseMessage('1. a\n2. b'))).toEqual([
      'orderedList',
      'listItem',
      'paragraph',
      'text',
      'listItem',
      'paragraph',
      'text',
    ]);
    expect(kinds(parseMessage('a\n\n---\n\nb'))).toEqual(['paragraph', 'text', 'rule', 'paragraph', 'text']);
  });

  it('keeps the fence language', () => {
    const [block] = parseMessage('```ts\nlet a = 1;\n```');
    expect(block).toEqual({ kind: 'codeBlock', text: 'let a = 1;', language: 'ts' });
  });

  it('treats a backslash line break as a hard break', () => {
    expect(kinds(parseMessage('a\\\nb'))).toEqual(['paragraph', 'text', 'break', 'text']);
  });

  it('decodes the entities the serialiser writes', () => {
    expect(textOf(parseMessage('&lt;script&gt;'))).toBe('<script>');
  });

  it('understands the escapes the serialiser writes', () => {
    expect(textOf(parseMessage('a \\*b\\* \\_c\\_'))).toBe('a *b* _c_');
  });

  it('never produces a heading or a table, whatever the input', () => {
    expect(kinds(parseMessage('# not a heading'))).toEqual(['paragraph', 'text']);
    expect(new Set(kinds(parseMessage('| a | b |\n| - | - |\n| 1 | 2 |')))).toEqual(
      new Set(['paragraph', 'text']),
    );
  });

  it('degrades image syntax to a link, since there is no image node', () => {
    const nodes = parseMessage('![alt](https://x.test/a.png)');
    expect(kinds(nodes)).toEqual(['paragraph', 'text', 'link', 'text']);
    expect(textOf(nodes)).toBe('!alt');
  });

  it('does not parse raw html', () => {
    expect(textOf(parseMessage('<img src=x onerror=alert(1)>'))).toContain('<img');
    expect(kinds(parseMessage('<img src=x onerror=alert(1)>'))).toEqual(['paragraph', 'text']);
  });
});

describe('the link allowlist', () => {
  it('keeps http, https and mailto', () => {
    for (const href of ['https://x.test/a', 'http://x.test/a', 'mailto:a@x.test']) {
      const [paragraph] = parseMessage(`[click](${href})`);
      expect(paragraph).toMatchObject({
        kind: 'paragraph',
        children: [{ kind: 'link', href }],
      });
    }
  });

  it('renders a rejected protocol as text, not as an anchor', () => {
    for (const href of [
      'javascript:alert(1)',
      'data:text/html;base64,PHNjcmlwdD4=',
      'vbscript:msgbox(1)',
      'file:///etc/passwd',
    ]) {
      const nodes = parseMessage(`[click](${href})`);
      expect(kinds(nodes)).not.toContain('link');
      expect(textOf(nodes)).toContain('click');
    }
  });

  it('rejects a protocol hidden behind whitespace or case', () => {
    for (const href of ['JaVaScRiPt:alert(1)', ' javascript:alert(1)', 'java\tscript:alert(1)']) {
      expect(kinds(parseMessage(`[click](${href})`))).not.toContain('link');
    }
  });

  it('autolinks a bare url the way the display path did', () => {
    const [paragraph] = parseMessage('see https://x.test/a now');
    expect(paragraph).toMatchObject({
      kind: 'paragraph',
      children: [{ kind: 'text' }, { kind: 'link', href: 'https://x.test/a' }, { kind: 'text' }],
    });
  });
});
