import { useState } from 'react';
import { avatarColorFor, initialOf } from '@/lib/userHelpers';
import { useMediaUrl } from '@/shared/hooks/useMediaUrl';

export type AvatarSize = 'xs' | 'sm' | 'md' | 'lg' | 'xl' | '2xl';

interface AvatarProps {
  userId: string;
  name: string;
  avatarUrl?: string | null;
  size?: AvatarSize;
  className?: string;
}

const SIZES: Record<AvatarSize, string> = {
  xs: 'w-6 h-6 text-3xs',
  sm: 'w-7 h-7 text-xs',
  md: 'w-8 h-8 text-sm',
  lg: 'w-10 h-10 text-sm',
  xl: 'w-16 h-16 text-2xl',
  '2xl': 'w-28 h-28 text-4xl',
};

export function Avatar({ userId, name, avatarUrl, size = 'md', className = '' }: AvatarProps) {
  const [brokenUrl, setBrokenUrl] = useState<string | null>(null);
  const src = useMediaUrl(avatarUrl);
  const shape = `${SIZES[size]} rounded-full shrink-0 ${className}`;

  if (src && src !== brokenUrl) {
    return (
      <img
        src={src}
        alt={name}
        data-qa="avatar-image"
        onError={() => setBrokenUrl(src)}
        className={`${shape} object-cover bg-raised`}
      />
    );
  }

  return (
    <span
      role="img"
      aria-label={name}
      data-qa="avatar-initials"
      className={`${shape} ${avatarColorFor(userId)} inline-flex items-center justify-center font-bold text-white select-none`}
    >
      {initialOf(name)}
    </span>
  );
}
