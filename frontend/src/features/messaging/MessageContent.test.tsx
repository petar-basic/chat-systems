import { describe, it, expect, vi } from 'vitest';
import { render } from '@testing-library/react';
import MessageContent from './MessageContent';

const SELF_ID = 'user-self';

vi.mock('@/hooks/queries/useAuth', () => ({
  useCurrentUser: () => ({ data: { id: SELF_ID } }),
}));

function renderContent(content: string) {
  const { container } = render(<MessageContent content={content} />);
  return container.querySelector('.tiptap-content') as HTMLElement;
}

describe('MessageContent', () => {
  it('renders the elements the stylesheet targets', () => {
    const root = renderContent('**b** *i* ~~s~~ `c`');
    expect(root.querySelector('strong')).toHaveTextContent('b');
    expect(root.querySelector('em')).toHaveTextContent('i');
    expect(root.querySelector('s')).toHaveTextContent('s');
    expect(root.querySelector('code')).toHaveTextContent('c');
  });

  it('renders blocks with the same tags the editor produced', () => {
    expect(renderContent('> quoted').querySelector('blockquote')).toHaveTextContent('quoted');
    expect(renderContent('- a\n- b').querySelectorAll('ul li')).toHaveLength(2);
    expect(renderContent('1. a\n2. b').querySelectorAll('ol li')).toHaveLength(2);
    expect(renderContent('a\n\n---\n\nb').querySelector('hr')).toBeInTheDocument();
    const pre = renderContent('```ts\nlet a = 1;\n```').querySelector('pre code');
    expect(pre).toHaveClass('language-ts');
    expect(pre).toHaveTextContent('let a = 1;');
  });

  it('keeps the link policy', () => {
    const link = renderContent('[click](https://x.test/a)').querySelector('a');
    expect(link).toHaveAttribute('href', 'https://x.test/a');
    expect(link).toHaveAttribute('target', '_blank');
    expect(link).toHaveAttribute('rel', 'noopener noreferrer nofollow');
  });

  it('renders a hostile protocol as text with no anchor at all', () => {
    for (const href of ['javascript:alert(1)', 'data:text/html,<script>alert(1)</script>']) {
      const root = renderContent(`[click](${href})`);
      expect(root.querySelector('a')).toBeNull();
      expect(root).toHaveTextContent('click');
    }
  });

  it('never injects markup from the message body', () => {
    const root = renderContent('<img src=x onerror=alert(1)>');
    expect(root.querySelector('img')).toBeNull();
    expect(root).toHaveTextContent('<img src=x onerror=alert(1)>');
  });

  it('highlights a mention of somebody else', () => {
    const root = renderContent('hi @[Ana Marija](user-ana) there');
    const pill = root.querySelector('.mention');
    expect(pill).toHaveTextContent('@Ana Marija');
    expect(pill).toHaveClass('mention-other');
  });

  it('highlights a mention of the current user differently', () => {
    const root = renderContent(`ping @[Me](${SELF_ID})`);
    expect(root.querySelector('.mention')).toHaveClass('mention-self');
  });

  it('treats broadcast mentions as self mentions', () => {
    for (const word of ['here', 'everyone', 'channel']) {
      const root = renderContent(`@${word} please look`);
      expect(root.querySelector('.mention')).toHaveClass('mention-self');
    }
  });

  it('does not highlight a broadcast word that runs into another word', () => {
    expect(renderContent('@channels are fine').querySelector('.mention')).toBeNull();
  });

  it('prefers the longest matching label', () => {
    const root = renderContent('@[Ana Marija](user-ana-marija) and @[Ana](user-ana)');
    const pills = root.querySelectorAll('.mention');
    expect(pills).toHaveLength(2);
    expect(pills[0]).toHaveTextContent('@Ana Marija');
    expect(pills[1]).toHaveTextContent('@Ana');
  });

  it('does not treat an email address as a mention', () => {
    const root = renderContent('write to ana@[Ana](user-ana)');
    expect(root.querySelector('.mention')).toBeNull();
  });

  /// A mention of a group you are in is a mention of you — that is the whole
  /// point of `@backend`.
  it('highlights a mention of your own group like your own name', async () => {
    const { useUserGroupStore } = await import('@/stores/userGroups');
    useUserGroupStore.getState().populate(['group:g1']);

    const { container } = render(<MessageContent content="@[backend](group:g1) and @[frontend](group:g2)" />);

    expect(container.querySelector('.mention-self')).toHaveTextContent('@backend');
    expect(container.querySelector('.mention-other')).toHaveTextContent('@frontend');

    useUserGroupStore.getState().populate([]);
  });
});
