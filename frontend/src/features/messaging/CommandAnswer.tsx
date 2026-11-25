import { useTranslation } from 'react-i18next';

interface Props {
  text: string | null;
  pending: boolean;
  onDismiss: () => void;
}

export function CommandAnswer({ text, pending, onDismiss }: Props) {
  const { t } = useTranslation('messages');
  if (!text) return null;
  return (
    <div
      data-qa="command-response"
      data-pending={pending || undefined}
      role="status"
      className="mx-4 mb-2 px-3 py-2 rounded-lg bg-surface/80 border border-line text-sm text-fg-dim flex items-start gap-2"
    >
      <span className={`flex-1 whitespace-pre-wrap ${pending ? 'italic animate-pulse' : ''}`}>{text}</span>
      <button
        type="button"
        onClick={onDismiss}
        aria-label={t('commandAnswer.dismiss')}
        data-tooltip={t('commandAnswer.dismiss')}
        className="text-subtle hover:text-fg transition cursor-pointer"
      >
        ×
      </button>
    </div>
  );
}
