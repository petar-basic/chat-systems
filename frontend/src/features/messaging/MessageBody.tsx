import type { MutableRefObject, ReactNode } from 'react';
import type { Message } from '@/stores/workspace';
import RichTextDisplay from '@/features/messaging/RichTextDisplay';
import { PollCard, type PollKeyVote } from '@/features/polls';
import { FormCard, FormAnswers, introOf } from '@/features/workflows';
import { LinkPreviews } from './LinkPreviews';
import { AppConnectPrompt } from './AppConnectPrompt';

interface Props {
  message: Message;
  currentUserId: string;
  pollKeyVote: MutableRefObject<PollKeyVote | null>;
  markers?: ReactNode;
  onHidePreviews?: (messageId: string) => void;
  onRefreshPreviews?: (messageId: string) => void;
}

export function MessageBody({
  message,
  currentUserId,
  pollKeyVote,
  markers,
  onHidePreviews,
  onRefreshPreviews,
}: Props) {
  const poll = message.metadata?.poll ?? null;
  const form = message.metadata?.form ?? null;
  const answers = message.deleted_at ? null : (message.metadata?.fields ?? null);
  const intro = answers ? introOf(message.content, answers) : null;
  const attachments = message.metadata?.attachments ?? [];

  return (
    <>
      {poll ? (
        <PollCard message={message} poll={poll} currentUserId={currentUserId} keyVoteRef={pollKeyVote} />
      ) : form && !message.deleted_at ? (
        <FormCard form={form} currentUserId={currentUserId} />
      ) : answers && intro !== null ? (
        <>
          <RichTextDisplay content={intro} attachments={attachments} />
          <FormAnswers fields={answers} />
        </>
      ) : (
        <RichTextDisplay content={message.content} attachments={attachments} />
      )}
      {!message.deleted_at && !!message.metadata?.unfurls?.length && (
        <LinkPreviews
          unfurls={message.metadata.unfurls}
          onHide={
            onHidePreviews && message.user_id === currentUserId ? () => onHidePreviews(message.id) : undefined
          }
          onRefresh={
            onRefreshPreviews && message.user_id === currentUserId
              ? () => onRefreshPreviews(message.id)
              : undefined
          }
        />
      )}
      {!message.deleted_at &&
        message.user_id === currentUserId &&
        !!message.metadata?.app_prompts?.length && (
          <AppConnectPrompt providers={message.metadata.app_prompts} messageId={message.id} />
        )}
      {markers && <div className="flex items-center gap-2">{markers}</div>}
    </>
  );
}
