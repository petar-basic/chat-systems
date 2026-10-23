export function fieldStyles({
  invalid = false,
  leading = false,
  trailing = false,
}: { invalid?: boolean; leading?: boolean; trailing?: boolean } = {}) {
  return `w-full rounded-lg border bg-raised/50 text-sm text-fg placeholder-muted transition focus:outline-none focus:ring-2 disabled:cursor-not-allowed disabled:opacity-60 ${
    leading ? 'pl-8' : 'pl-3'
  } ${trailing ? 'pr-8' : 'pr-3'} ${invalid ? 'border-red-500/60 focus:ring-red-500' : 'border-line-strong focus:ring-purple-500'}`;
}
