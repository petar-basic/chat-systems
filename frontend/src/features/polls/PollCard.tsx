import { useEffect, type RefObject } from 'react';
import { BarChart3, Check } from 'lucide-react';
import type { Message } from '@/stores/workspace';
import { useWorkspaceStore } from '@/stores/workspace';
import { useUserCache } from '@/stores/users';
import { displayNameOf } from '@/lib/userHelpers';
import { formatDateTime } from '@/lib/datetime';
import { Avatar } from '@/shared/components/Avatar/Avatar';
import { Button } from '@/shared/components/Button/Button';
import { useClosePoll, useMyVote, usePollVoters, useVote, type PollSummary } from './usePolls';

const SHOWN_VOTERS = 3;

export type PollKeyVote = (index: number) => void;

interface Props {
  message: Message;
  poll: PollSummary;
  currentUserId: string;
  keyVoteRef?: RefObject<PollKeyVote | null>;
}

function nextChoice(poll: PollSummary, mine: string[], optionId: string): string[] {
  if (mine.includes(optionId)) return mine.filter((id) => id !== optionId);
  if (!poll.multiple_choice) return [optionId];
  return poll.options.map((option) => option.id).filter((id) => id === optionId || mine.includes(id));
}

function footerText(poll: PollSummary): string {
  const votes = `${poll.voters} ${poll.voters === 1 ? 'vote' : 'votes'}`;
  if (poll.closed_at) return `${votes} · Closed`;
  if (poll.closes_at) return `${votes} · closes ${formatDateTime(poll.closes_at)}`;
  return votes;
}

export function PollCard({ message, poll, currentUserId, keyVoteRef }: Props) {
  const instanceUrl = useWorkspaceStore((s) => s.currentWorkspace?.instanceUrl);
  const { getUser } = useUserCache();
  const mine = useMyVote(poll.id, instanceUrl).data ?? [];
  const voters = usePollVoters(poll, instanceUrl).data;
  const vote = useVote(message, poll, instanceUrl);
  const close = useClosePoll(message, poll.id, instanceUrl);
  const closed = !!poll.closed_at;

  const choose = (optionId: string) => {
    if (closed) return;
    vote.mutate(nextChoice(poll, mine, optionId));
  };

  useEffect(() => {
    if (!keyVoteRef) return undefined;
    keyVoteRef.current = closed
      ? null
      : (index) => {
          const option = poll.options[index];
          if (option) vote.mutate(nextChoice(poll, mine, option.id));
        };
    return () => {
      keyVoteRef.current = null;
    };
  });

  return (
    <div
      data-qa="poll-card"
      className="mt-1 max-w-md rounded-xl border border-line bg-surface p-3"
      aria-label={`Poll: ${poll.question}`}
      role="group"
    >
      <div className="flex items-start gap-2">
        <BarChart3 className="mt-0.5 h-4 w-4 shrink-0 text-accent-soft" aria-hidden="true" />
        <div className="min-w-0">
          <p className="text-sm font-semibold text-fg break-words" data-qa="poll-question">
            {poll.question}
          </p>
          <p className="text-xs text-muted">
            {poll.multiple_choice ? 'Choose any' : 'Choose one'}
            {poll.anonymous ? ' · Anonymous' : ''}
          </p>
        </div>
      </div>

      <ul className="mt-2 space-y-1.5">
        {poll.options.map((option, index) => {
          const chosen = mine.includes(option.id);
          const share = poll.voters > 0 ? Math.round((option.votes / poll.voters) * 100) : 0;
          const who = voters?.find((entry) => entry.option_id === option.id)?.user_ids ?? [];
          return (
            <li key={option.id}>
              <button
                type="button"
                aria-pressed={chosen}
                disabled={closed}
                onClick={() => choose(option.id)}
                data-qa="poll-option"
                title={closed ? undefined : `${index + 1} on the keyboard`}
                className={`relative flex w-full items-center gap-2 overflow-hidden rounded-lg border px-3 py-2 text-left text-sm transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-purple-500 disabled:cursor-default ${
                  chosen
                    ? 'border-purple-500 text-fg'
                    : 'border-line-strong text-fg-soft enabled:hover:bg-raised/40'
                }`}
              >
                <span
                  aria-hidden="true"
                  className={`absolute inset-y-0 left-0 ${chosen ? 'bg-purple-600/20' : 'bg-raised/60'}`}
                  style={{ width: `${share}%` }}
                />
                <span className="relative flex h-4 w-4 shrink-0 items-center justify-center">
                  {chosen && <Check className="h-4 w-4 text-accent-soft" data-qa="poll-option-mine" />}
                </span>
                <span className="relative min-w-0 flex-1 break-words">{option.label}</span>
                {who.length > 0 && (
                  <span className="relative flex -space-x-1.5" data-qa="poll-option-voters">
                    {who.slice(0, SHOWN_VOTERS).map((userId) => {
                      const user = getUser(userId);
                      return (
                        <Avatar
                          key={userId}
                          userId={userId}
                          name={displayNameOf(user?.display_name)}
                          avatarUrl={user?.avatar_url}
                          size="xs"
                          className="ring-2 ring-surface"
                        />
                      );
                    })}
                  </span>
                )}
                <span className="relative w-14 shrink-0 text-right text-xs tabular-nums text-muted">
                  <span data-qa="poll-option-count">{option.votes}</span> · {share}%
                </span>
              </button>
            </li>
          );
        })}
      </ul>

      <div className="mt-2 flex items-center justify-between gap-2">
        <span className="text-xs text-muted" data-qa="poll-footer">
          {footerText(poll)}
        </span>
        {!closed && poll.created_by === currentUserId && (
          <Button size="sm" variant="ghost" loading={close.isPending} onClick={() => close.mutate()}>
            Close poll
          </Button>
        )}
      </div>
    </div>
  );
}
