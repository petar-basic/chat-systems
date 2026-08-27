/**
 * Where the reader stopped. The sidebar count says how many are new; this says
 * which ones — without it, opening a channel with forty unread messages means
 * scrolling and guessing.
 */
export function UnreadDivider() {
  return (
    <div className="flex items-center gap-2 px-2 py-1" data-qa="unread-divider">
      <div className="h-px flex-1 bg-red-500/60" />
      <span className="text-[10px] font-semibold uppercase tracking-wide text-red-400">New</span>
      <div className="h-px w-6 bg-red-500/60" />
    </div>
  );
}
