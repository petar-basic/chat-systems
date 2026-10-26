import { useState } from 'react';
import type { Message, WorkspaceMember, Channel } from '@/stores/workspace';
import MessageInput from './MessageInput';
import { MessageAttachments } from './MessageAttachments';

interface Props {
  message: Message;
  members?: WorkspaceMember[];
  channels?: Channel[];
  onSave: (content: string, keptAttachmentIds?: string[]) => Promise<void>;
  onCancel: () => void;
}

export function MessageEditor({ message, members, channels, onSave, onCancel }: Props) {
  const [keptAttachments, setKeptAttachments] = useState<string[] | null>(null);
  const attachments = message.metadata?.attachments ?? [];
  const editingAttachments = attachments.filter((a) => keptAttachments?.includes(a.id) ?? true);

  return (
    <>
      <MessageInput
        key={`edit:${message.id}`}
        editing
        allowEmpty={editingAttachments.length > 0}
        initialContent={message.content}
        members={members}
        channels={channels}
        onSend={(content) =>
          onSave(
            content,
            keptAttachments && keptAttachments.length !== attachments.length ? keptAttachments : undefined,
          )
        }
        onCancel={onCancel}
      />
      {editingAttachments.length > 0 && (
        <MessageAttachments
          attachments={editingAttachments}
          onRemove={(id) =>
            setKeptAttachments((current) =>
              (current ?? attachments.map((a) => a.id)).filter((kept) => kept !== id),
            )
          }
        />
      )}
    </>
  );
}
