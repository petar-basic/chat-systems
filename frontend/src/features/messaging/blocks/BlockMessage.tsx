import type { Message } from '@/stores/workspace';
import MessageContent from '../MessageContent';
import { MessageAttachments } from '../MessageAttachments';
import { BlockView } from './BlockView';
import { LegacyAttachmentView } from './LegacyAttachmentView';
import { BlockActionsContext } from './blockActions';
import { useMessageActions } from './interactivity';

interface Props {
  message: Message;
}

export function BlockMessage({ message }: Props) {
  const blocks = message.metadata?.blocks ?? [];
  const attachments = message.metadata?.legacy_attachments ?? [];
  const files = message.metadata?.attachments ?? [];
  const showText = blocks.length === 0 && !message.metadata?.flattened && !!message.content.trim();
  const actions = useMessageActions(message);
  return (
    <BlockActionsContext.Provider value={actions}>
      <div className="flex max-w-2xl flex-col gap-2" data-qa="block-message">
        {showText && <MessageContent content={message.content} />}
        {blocks.map((block, index) => (
          <BlockView key={index} block={block} />
        ))}
        {attachments.map((attachment, index) => (
          <LegacyAttachmentView key={index} attachment={attachment} />
        ))}
        {files.length > 0 && <MessageAttachments attachments={files} />}
      </div>
    </BlockActionsContext.Provider>
  );
}
