import { memo, useMemo, type ReactNode } from 'react';
import { parseMessage, type MessageNode } from '@/lib/messageMarkdown';
import { highlightMentions, type MentionRef } from '@/lib/mentionHighlight';
import { parseMentions, flattenMentions } from '@/lib/mentions';
import { useCurrentUser } from '@/hooks/queries/useAuth';

interface Props {
  content: string;
  className?: string;
}

interface RenderContext {
  selfId: string | undefined;
  mentions: MentionRef[];
}

function renderText(text: string, ctx: RenderContext, key: string): ReactNode {
  const spans = highlightMentions(text, ctx.selfId, ctx.mentions);
  if (spans.length === 1 && spans[0].mention === null) return spans[0].text;
  return spans.map((span, i) =>
    span.mention === null ? (
      <span key={`${key}-${i}`}>{span.text}</span>
    ) : (
      <span key={`${key}-${i}`} className={`mention mention-${span.mention}`}>
        {span.text}
      </span>
    ),
  );
}

function renderNodes(nodes: MessageNode[], ctx: RenderContext, keyPrefix: string): ReactNode[] {
  return nodes.map((node, i) => renderNode(node, ctx, `${keyPrefix}.${i}`));
}

function renderNode(node: MessageNode, ctx: RenderContext, key: string): ReactNode {
  switch (node.kind) {
    case 'text':
      return <span key={key}>{renderText(node.text, ctx, key)}</span>;
    case 'break':
      return <br key={key} />;
    case 'rule':
      return <hr key={key} />;
    case 'code':
      return <code key={key}>{node.text}</code>;
    case 'codeBlock':
      return (
        <pre key={key}>
          <code className={node.language ? `language-${node.language}` : undefined}>{node.text}</code>
        </pre>
      );
    case 'fragment':
      return <span key={key}>{renderNodes(node.children, ctx, key)}</span>;
    case 'link':
      return (
        <a key={key} href={node.href} target="_blank" rel="noopener noreferrer nofollow">
          {renderNodes(node.children, ctx, key)}
        </a>
      );
    case 'strong':
      return <strong key={key}>{renderNodes(node.children, ctx, key)}</strong>;
    case 'em':
      return <em key={key}>{renderNodes(node.children, ctx, key)}</em>;
    case 'strike':
      return <s key={key}>{renderNodes(node.children, ctx, key)}</s>;
    case 'paragraph':
      return <p key={key}>{renderNodes(node.children, ctx, key)}</p>;
    case 'blockquote':
      return <blockquote key={key}>{renderNodes(node.children, ctx, key)}</blockquote>;
    case 'bulletList':
      return <ul key={key}>{renderNodes(node.children, ctx, key)}</ul>;
    case 'orderedList':
      return (
        <ol key={key} start={node.start === 1 ? undefined : node.start}>
          {renderNodes(node.children, ctx, key)}
        </ol>
      );
    case 'listItem':
      return <li key={key}>{renderNodes(node.children, ctx, key)}</li>;
  }
}

/**
 * Renders a stored message without an editor. The previous implementation
 * mounted a TipTap instance per message — a ProseMirror state, view, plugin
 * stack and contenteditable subtree — to display text nobody edits.
 *
 * The tree maps to elements directly. There is no `dangerouslySetInnerHTML`
 * anywhere on this path, so the XSS posture holds by construction rather than by
 * getting a sanitiser's configuration right.
 */
function MessageContent({ content, className }: Props) {
  const { data: user } = useCurrentUser();
  const selfId = user?.id;

  const nodes = useMemo(() => parseMessage(flattenMentions(content)), [content]);
  const mentions = useMemo(() => parseMentions(content), [content]);

  const rendered = useMemo(() => renderNodes(nodes, { selfId, mentions }, 'n'), [nodes, selfId, mentions]);

  return <div className={`tiptap-content${className ? ` ${className}` : ''}`}>{rendered}</div>;
}

export default memo(MessageContent);
