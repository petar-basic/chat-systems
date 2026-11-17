import type { components } from '@/api/schema';
import MessageContent from '../MessageContent';

type BlockText = components['schemas']['BlockText'];

interface Props {
  text: BlockText;
  className?: string;
}

export function BlockTextView({ text, className }: Props) {
  if (text.type === 'mrkdwn') return <MessageContent content={text.text} className={className} />;
  return <span className={`whitespace-pre-wrap ${className ?? ''}`}>{text.text}</span>;
}
