import type { components } from '@/api/schema';
import { MessageAttachments } from './MessageAttachments';
import MessageContent from './MessageContent';

type Attachment = components['schemas']['Attachment'];

interface Props {
  content: string;
  attachments?: Attachment[] | null;
  compact?: boolean;
  className?: string;
}

export default function RichTextDisplay({ content, attachments, compact, className }: Props) {
  return (
    <>
      {content.trim() && <MessageContent content={content} className={className} />}
      {!!attachments?.length && <MessageAttachments attachments={attachments} compact={compact} />}
    </>
  );
}
