import { HoverCard } from '@/shared/components/Popover/HoverCard';
import { ReactionEmoji } from '@/shared/components/ReactionEmoji';
import type { Message } from '@/stores/workspace';
import { useUserCache } from '@/stores/users';
import { displayNameOf } from '@/lib/userHelpers';
import { joinReactors, reactorNames, type ReactionGroup } from './reactionSummary';

function groupReactions(message: Message, currentUserId: string): ReactionGroup[] {
  const groups: ReactionGroup[] = [];
  for (const r of message.reactions ?? []) {
    const existing = groups.find((g) => g.emoji === r.emoji);
    if (existing) {
      existing.count++;
      existing.userIds.push(r.user_id);
      if (r.user_id === currentUserId) existing.hasOwn = true;
    } else {
      groups.push({
        emoji: r.emoji,
        count: 1,
        hasOwn: r.user_id === currentUserId,
        userIds: [r.user_id],
      });
    }
  }
  return groups;
}

export function MessageReactions({
  message,
  currentUserId,
  onToggle,
}: {
  message: Message;
  currentUserId: string;
  onToggle: (emoji: string) => void;
}) {
  const groups = groupReactions(message, currentUserId);
  if (groups.length === 0) return null;
  return (
    <div className="flex flex-wrap gap-1 mt-1">
      {groups.map((g) => (
        <ReactionPill key={g.emoji} group={g} currentUserId={currentUserId} onToggle={onToggle} />
      ))}
    </div>
  );
}

function ReactionPill({
  group,
  currentUserId,
  onToggle,
}: {
  group: ReactionGroup;
  currentUserId: string;
  onToggle: (emoji: string) => void;
}) {
  const getUser = useUserCache((s) => s.getUser);
  const names = reactorNames(group.userIds, currentUserId, (id) => {
    const user = getUser(id);
    return user ? displayNameOf(user.display_name) : undefined;
  });

  return (
    <HoverCard
      placement="top"
      dataQa="reaction-tooltip"
      triggerClassName="inline-flex"
      panelClassName="max-w-56 px-2.5 py-1.5 rounded-lg bg-elevated border border-line-strong shadow-xl text-xs text-fg-dim text-center"
      content={
        <>
          <span className="block text-lg leading-none mb-1">
            <ReactionEmoji emoji={group.emoji} />
          </span>
          <span className="text-fg-soft font-medium">{joinReactors(names)}</span>
          <span> reacted</span>
        </>
      }
    >
      <button
        onClick={() => onToggle(group.emoji)}
        aria-pressed={group.hasOwn}
        aria-label={`${joinReactors(names)} reacted`}
        data-qa="message-reaction"
        className={`inline-flex items-center gap-1 px-1.5 py-0.5 rounded-md text-xs border transition ${
          group.hasOwn
            ? 'bg-purple-600/20 border-purple-500/40 text-accent-soft'
            : 'bg-raised/50 border-line-strong/50 text-fg-dim hover:bg-raised'
        }`}
      >
        <ReactionEmoji emoji={group.emoji} />
        <span>{group.count}</span>
      </button>
    </HoverCard>
  );
}
