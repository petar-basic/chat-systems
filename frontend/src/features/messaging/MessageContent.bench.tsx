import type { ComponentType } from 'react';
import { bench, describe } from 'vitest';
import { render, cleanup } from '@testing-library/react';
import { useEditor, EditorContent } from '@tiptap/react';
import StarterKit from '@tiptap/starter-kit';
import Underline from '@tiptap/extension-underline';
import Link from '@tiptap/extension-link';
import { Markdown } from 'tiptap-markdown';
import MessageContent from './MessageContent';
import { flattenMentions } from '@/lib/mentions';

/**
 * The renderer CS-024 replaced, reconstructed here so the comparison is against
 * what actually shipped rather than against a guess. Run with `npx vitest bench`.
 */
function LegacyMessage({ content }: { content: string }) {
  const editor = useEditor({
    editable: false,
    extensions: [
      StarterKit.configure({ heading: false, link: false, underline: false }),
      Underline,
      Link.configure({
        openOnClick: true,
        autolink: true,
        protocols: ['http', 'https', 'mailto'],
        HTMLAttributes: { rel: 'noopener noreferrer nofollow', target: '_blank' },
      }),
      Markdown.configure({ html: false }),
    ],
    content: flattenMentions(content) || '',
  });
  return <EditorContent editor={editor} className="tiptap-content" />;
}

const SAMPLES = [
  'just a plain line of chat',
  '**bold** and *italic* and ~~struck~~ with `code`',
  'ping @[Ana Marija](user-ana) about https://example.test/a/b',
  '- one\n- two\n- three',
  '> quoted reply\n\nand a follow-up',
  '```ts\nconst x: number = 1;\nconsole.log(x);\n```',
];

function corpus(count: number) {
  return Array.from({ length: count }, (_, i) => SAMPLES[i % SAMPLES.length]);
}

function List({ items, Row }: { items: string[]; Row: ComponentType<{ content: string }> }) {
  return (
    <div>
      {items.map((content, i) => (
        <Row key={i} content={content} />
      ))}
    </div>
  );
}

for (const size of [100, 500]) {
  describe(`mounting ${size} messages`, () => {
    const items = corpus(size);

    bench(
      'MessageContent (static tree)',
      () => {
        render(<List items={items} Row={MessageContent} />);
        cleanup();
      },
      { iterations: 5 },
    );

    bench(
      'RichTextDisplay (one TipTap editor per message)',
      () => {
        render(<List items={items} Row={LegacyMessage} />);
        cleanup();
      },
      { iterations: 5 },
    );
  });
}
