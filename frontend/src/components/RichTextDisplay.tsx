import { parseAttachment } from '../lib/attachments';
import { AttachmentCard } from '../features/messaging/AttachmentCard';
import MessageContent from '../features/messaging/MessageContent';

interface Props {
  content: string;
  className?: string;
}

export default function RichTextDisplay({ content, className }: Props) {
  const attachment = parseAttachment(content);
  if (attachment) return <AttachmentCard {...attachment} />;
  return <MessageContent content={content} className={className} />;
}
