import type { ButtonHTMLAttributes, ReactNode } from 'react';

type Tone = 'default' | 'danger' | 'inverse';

const TONES: Record<Tone, string> = {
  default: 'text-muted hover:text-fg hover:bg-raised/60 aria-pressed:bg-raised aria-pressed:text-fg',
  danger: 'text-muted hover:text-danger hover:bg-raised/60',
  inverse: 'text-white/70 hover:text-white hover:bg-white/10',
};

const SIZES = {
  sm: 'h-7 w-7',
  md: 'h-8 w-8',
};

export interface IconButtonProps extends Omit<
  ButtonHTMLAttributes<HTMLButtonElement>,
  'children' | 'aria-label' | 'title'
> {
  label: string;
  icon: ReactNode;
  size?: 'sm' | 'md';
  tone?: Tone;
  pressed?: boolean;
}

export function IconButton({
  label,
  icon,
  size = 'sm',
  tone = 'default',
  pressed,
  className = '',
  type = 'button',
  ...rest
}: IconButtonProps) {
  return (
    <button
      type={type}
      aria-label={label}
      title={label}
      aria-pressed={pressed}
      className={`inline-flex shrink-0 items-center justify-center rounded-md transition cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-purple-500 disabled:cursor-not-allowed disabled:opacity-50 ${TONES[tone]} ${SIZES[size]} ${className}`}
      {...rest}
    >
      {icon}
    </button>
  );
}
