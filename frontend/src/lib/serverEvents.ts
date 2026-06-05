import type { Message, Reaction } from '@/stores/workspace';
import type { DirectMessage } from '@/hooks/queries/useDm';

export type PresenceValue = 'online' | 'away' | 'offline';

export type ReactionEvent = Reaction & { channel_id: string };

export type AppServerEvent =
  | { type: 'workspace.created' }
  | { type: 'workspace.updated'; workspace_id: string }
  | { type: 'workspace.deleted'; workspace_id: string }
  | { type: 'workspace.restored' }
  | { type: 'member.added'; workspace_id: string }
  | { type: 'member.removed'; workspace_id: string }
  | { type: 'channel.created'; workspace_id: string }
  | { type: 'channel.updated'; channel_id: string }
  | { type: 'channel.member_added'; channel_id: string }
  | { type: 'channel.member_removed'; channel_id: string }
  | { type: 'message.new'; message: Message }
  | { type: 'message.updated'; message: Message }
  | { type: 'message.deleted'; message_id: string; channel_id: string }
  | { type: 'message.pinned'; message_id: string; channel_id: string; pinned: boolean }
  | { type: 'reaction.added'; message_id: string; reaction: ReactionEvent }
  | { type: 'reaction.removed'; message_id: string; channel_id: string; user_id: string; emoji: string }
  | { type: 'dm.new'; message: DirectMessage }
  | { type: 'dm.updated'; message: DirectMessage }
  | { type: 'dm.deleted'; message: DirectMessage }
  | {
      type: 'dm.reaction.added';
      message_id: string;
      workspace_id: string;
      from_user_id: string;
      to_user_id: string;
      reaction: Reaction;
    }
  | {
      type: 'dm.reaction.removed';
      message_id: string;
      workspace_id: string;
      from_user_id: string;
      to_user_id: string;
      user_id: string;
      emoji: string;
    }
  | {
      type: 'notification';
      notification_type?: string;
      channel_id?: string;
      message_id?: string;
      workspace_id?: string;
      title: string;
      body: string;
      priority?: string;
    }
  | { type: 'presence.changed'; user_id: string; status: PresenceValue }
  | { type: 'presence.batch'; users: Array<{ user_id: string; status: PresenceValue }> }
  | { type: 'typing.indicator'; channel_id: string; user_id: string; is_typing: boolean };

export type ServerEventType = AppServerEvent['type'];
export type EventOfType<T extends ServerEventType> = Extract<AppServerEvent, { type: T }>;
