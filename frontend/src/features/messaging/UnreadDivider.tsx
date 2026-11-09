import { useTranslation } from 'react-i18next';

/**
 * Where the reader stopped. The sidebar count says how many are new; this says
 * which ones — without it, opening a channel with forty unread messages means
 * scrolling and guessing.
 */
export function UnreadDivider() {
  const { t } = useTranslation('messages');
  return (
    <div className="flex items-center gap-2 px-2 py-1" data-qa="unread-divider">
      <div className="h-px flex-1 bg-red-500/60" />
      <span className="text-3xs font-semibold uppercase tracking-wide text-danger">
        {t('list.newDivider')}
      </span>
      <div className="h-px w-6 bg-red-500/60" />
    </div>
  );
}
