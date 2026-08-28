import { useState } from 'react';
import { avatarColorFor, initialOf } from '@/lib/userHelpers';

export type AvatarSize = 'xs' | 'sm' | 'md' | 'lg' | 'xl';

interface AvatarProps {
  userId: string;
  name: string;
  avatarUrl?: string | null;
  size?: AvatarSize;
  className?: string;
}

const SIZES: Record<AvatarSize, string> = {
  xs: 'w-6 h-6 text-[10px]',
  sm: 'w-7 h-7 text-xs',
  md: 'w-8 h-8 text-sm',
  lg: 'w-10 h-10 text-sm',
  xl: 'w-16 h-16 text-2xl',
};

export function Avatar({ userId, name, avatarUrl, size = 'md', className = '' }: AvatarProps) {
  const [brokenUrl, setBrokenUrl] = useState<string | null>(null);
  const shape = `${SIZES[size]} rounded-full shrink-0 ${className}`;

  if (avatarUrl && avatarUrl !== brokenUrl) {
    return (
      <img
        src={avatarUrl}
        alt={name}
        data-qa="avatar-image"
        onError={() => setBrokenUrl(avatarUrl)}
        className={`${shape} object-cover bg-raised`}
      />
    );
  }

  return (
    <span
      role="img"
      aria-label={name}
      data-qa="avatar-initials"
      className={`${shape} ${avatarColorFor(userId)} inline-flex items-center justify-center font-bold text-fg select-none`}
    >
      {initialOf(name)}
    </span>
  );
}
