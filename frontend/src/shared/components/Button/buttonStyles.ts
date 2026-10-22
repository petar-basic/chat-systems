export type ButtonVariant = 'primary' | 'secondary' | 'ghost' | 'danger';
export type ButtonSize = 'sm' | 'md';

const VARIANTS: Record<ButtonVariant, string> = {
  primary: 'bg-purple-600 text-white hover:bg-purple-500 disabled:hover:bg-purple-600',
  secondary: 'bg-raised text-fg-soft hover:bg-elevated',
  ghost: 'text-muted hover:text-fg hover:bg-raised/60',
  danger: 'bg-red-600 text-white hover:bg-red-500 disabled:hover:bg-red-600',
};

const SIZES: Record<ButtonSize, string> = {
  sm: 'h-7 px-2.5 gap-1.5 text-xs',
  md: 'h-9 px-4 gap-2 text-sm',
};

export function buttonStyles({
  variant = 'secondary',
  size = 'md',
  fullWidth = false,
}: { variant?: ButtonVariant; size?: ButtonSize; fullWidth?: boolean } = {}): string {
  return `inline-flex shrink-0 items-center justify-center rounded-lg font-medium transition cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-purple-500 disabled:cursor-not-allowed disabled:opacity-50 ${VARIANTS[variant]} ${SIZES[size]} ${fullWidth ? 'w-full' : ''}`;
}
