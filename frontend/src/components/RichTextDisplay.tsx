import { useEffect, useMemo, useRef } from 'react';
import { useEditor, EditorContent } from '@tiptap/react';
import { createDisplayExtensions } from '../lib/tiptapExtensions';
import { createMentionHighlightExtension, mentionHighlightPluginKey } from '../lib/mentionHighlightExtension';
import { parseMentions, flattenMentions } from '../lib/mentions';
import { useCurrentUser } from '../hooks/queries/useAuth';
import { parseAttachment } from '../lib/attachments';
import { AttachmentCard } from '../features/messaging/AttachmentCard';

interface Props {
  content: string;
  className?: string;
}

export default function RichTextDisplay({ content, className }: Props) {
  const attachment = parseAttachment(content);
  if (attachment) return <AttachmentCard {...attachment} />;
  return <MarkdownContent content={content} className={className} />;
}

function MarkdownContent({ content, className }: Props) {
  const lastContent = useRef(content);
  const { data: user } = useCurrentUser();

  const extensions = useMemo(() => [...createDisplayExtensions(), createMentionHighlightExtension()], []);

  const mentions = useMemo(() => parseMentions(content), [content]);
  const displayContent = useMemo(() => flattenMentions(content), [content]);

  const editor = useEditor({
    editable: false,
    extensions,
    content: displayContent || '',
  });

  useEffect(() => {
    if (editor && !editor.isDestroyed) {
      const { tr } = editor.state;
      tr.setMeta(mentionHighlightPluginKey, { selfId: user?.id, mentions });
      editor.view.dispatch(tr);
    }
  }, [editor, user?.id, mentions]);

  useEffect(() => {
    if (editor && content !== lastContent.current) {
      lastContent.current = content;
      editor.commands.setContent(flattenMentions(content) || '');
    }
  }, [editor, content]);

  return <EditorContent editor={editor} className={`tiptap-content${className ? ` ${className}` : ''}`} />;
}
