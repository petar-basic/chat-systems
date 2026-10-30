import type { ReactNode } from 'react';
import type { components } from '@/api/schema';
import { MessageAttachments } from './MessageAttachments';
import MessageContent from './MessageContent';

type Attachment = components['schemas']['Attachment'];

interface Props {
  content: string;
  attachments?: Attachment[] | null;
  compact?: boolean;
  className?: string;
  trailing?: ReactNode;
}

export default function RichTextDisplay({ content, attachments, compact, className, trailing }: Props) {
  const hasText = !!content.trim();
  return (
    <>
      {hasText && <MessageContent content={content} className={className} trailing={trailing} />}
      {!!attachments?.length && <MessageAttachments attachments={attachments} compact={compact} />}
      {!hasText && trailing && <div className="flex items-center gap-2">{trailing}</div>}
    </>
  );
}
