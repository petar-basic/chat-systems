import { useTranslation } from 'react-i18next';
import { CopyField } from '@/shared/components/CopyField/CopyField';

interface Props {
  name: string;
  token: string;
  onDismiss: () => void;
}

export function TokenReveal({ name, token, onDismiss }: Props) {
  const { t } = useTranslation('workspace');
  return (
    <div className="rounded-lg border border-warning/40 bg-warning/5 p-3" data-qa="bot-app-token">
      <p className="text-sm text-fg-soft">{t('settings.apps.token.copyNow', { name })}</p>
      <CopyField
        label={t('settings.apps.token.label')}
        value={token}
        valueQa="bot-app-token-value"
        copyQa="bot-app-token-copy"
      />
      <button
        onClick={onDismiss}
        data-qa="bot-app-token-dismiss"
        className="mt-2 text-xs text-muted hover:text-fg transition cursor-pointer"
      >
        {t('settings.apps.token.stored')}
      </button>
    </div>
  );
}
