import { useState, useEffect, useCallback } from 'react';
import { globalEventBus, type ServerEvent } from '../lib/globalEventBus';
import { useUserCache } from '../stores/users';

interface Props {
  channelId: string;
  currentUserId: string;
}

interface TypingUser {
  userId: string;
  expiresAt: number;
}

export default function TypingIndicator({ channelId, currentUserId }: Props) {
  const [typingUsers, setTypingUsers] = useState<TypingUser[]>([]);
  const { getUser } = useUserCache();

  const handleTypingEvent = useCallback(
    (event: ServerEvent) => {
      if (event.type !== 'typing.indicator') return;
      if (event.channel_id !== channelId) return;
      if (event.user_id === currentUserId) return;

      const userId = event.user_id as string;
      const isTyping = event.is_typing as boolean;

      if (isTyping) {
        setTypingUsers((prev) => {
          const filtered = prev.filter((t) => t.userId !== userId);
          return [...filtered, { userId, expiresAt: Date.now() + 5000 }];
        });
      } else {
        setTypingUsers((prev) => prev.filter((t) => t.userId !== userId));
      }
    },
    [channelId, currentUserId],
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
  }, [channelId]);

  if (typingUsers.length === 0) return null;

  const names = typingUsers.map((t) => getUser(t.userId)?.display_name || t.userId.slice(0, 8)).slice(0, 3);

  let text: string;
  if (names.length === 1) {
    text = `${names[0]} is typing...`;
  } else if (names.length === 2) {
    text = `${names[0]} and ${names[1]} are typing...`;
  } else {
    text = `${names[0]} and ${names.length - 1} others are typing...`;
  }

  return (
    <div className="px-4 py-1 text-xs text-slate-400 flex items-center gap-2">
      <div className="flex gap-0.5">
        <span
          className="w-1.5 h-1.5 bg-slate-400 rounded-full animate-bounce"
          style={{ animationDelay: '0ms' }}
        />
        <span
          className="w-1.5 h-1.5 bg-slate-400 rounded-full animate-bounce"
          style={{ animationDelay: '150ms' }}
        />
        <span
          className="w-1.5 h-1.5 bg-slate-400 rounded-full animate-bounce"
          style={{ animationDelay: '300ms' }}
        />
      </div>
      {text}
    </div>
  );
}
