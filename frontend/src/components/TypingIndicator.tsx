import { useState, useEffect, useCallback } from 'react';
import { globalEventBus, type ServerEvent } from '../lib/globalEventBus';
import { useUserCache } from '../stores/users';
import { UNKNOWN_USER, TYPING_INDICATOR_TTL_MS } from '@/shared/constants';

interface Props {
  channelId?: string;
  conversationId?: string;
  currentUserId: string;
}

interface TypingUser {
  userId: string;
  expiresAt: number;
}

export default function TypingIndicator({ channelId, conversationId, currentUserId }: Props) {
  const scopeId = channelId ?? conversationId;
  const [typingUsers, setTypingUsers] = useState<TypingUser[]>([]);
  const { getUser } = useUserCache();

  const handleTypingEvent = useCallback(
    (event: ServerEvent) => {
      if (event.type !== 'typing.indicator') return;
      if ((event.channel_id ?? event.conversation_id) !== scopeId) return;
      if (event.user_id === currentUserId) return;

      const userId = event.user_id as string;
      const isTyping = event.is_typing as boolean;

      if (isTyping) {
        setTypingUsers((prev) => {
          const filtered = prev.filter((t) => t.userId !== userId);
          return [...filtered, { userId, expiresAt: Date.now() + TYPING_INDICATOR_TTL_MS }];
        });
      } else {
        setTypingUsers((prev) => prev.filter((t) => t.userId !== userId));
      }
    },
    [scopeId, currentUserId],
  );

  useEffect(() => {
    const unsub = globalEventBus.on('typing.indicator', handleTypingEvent);
    return () => {
      unsub();
    };
  }, [handleTypingEvent]);

  useEffect(() => {
    if (typingUsers.length === 0) return;
    const interval = setInterval(() => {
      setTypingUsers((prev) => prev.filter((t) => t.expiresAt > Date.now()));
    }, 1000);
    return () => clearInterval(interval);
  }, [typingUsers.length]);

  useEffect(() => {
    return () => {
      setTypingUsers([]);
    };
  }, [scopeId]);

  if (typingUsers.length === 0) return null;

  const names = typingUsers.map((t) => getUser(t.userId)?.display_name || UNKNOWN_USER).slice(0, 3);

  let text: string;
  if (names.length === 1) {
    text = `${names[0]} is typing...`;
  } else if (names.length === 2) {
    text = `${names[0]} and ${names[1]} are typing...`;
  } else {
    text = `${names[0]} and ${names.length - 1} others are typing...`;
  }

  return (
    <div className="px-4 py-1 text-xs text-muted flex items-center gap-2">
      <div className="flex gap-0.5">
        <span
          className="w-1.5 h-1.5 bg-subtle rounded-full animate-bounce"
          style={{ animationDelay: '0ms' }}
        />
        <span
          className="w-1.5 h-1.5 bg-subtle rounded-full animate-bounce"
          style={{ animationDelay: '150ms' }}
        />
        <span
          className="w-1.5 h-1.5 bg-subtle rounded-full animate-bounce"
          style={{ animationDelay: '300ms' }}
        />
      </div>
      {text}
    </div>
  );
}
