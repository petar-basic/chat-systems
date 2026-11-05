import { useTranslation } from 'react-i18next';
import { RadioCards } from '@/shared/components/Form/RadioCards';
import { toast } from '@/shared/components/Toast';
import { useWorkspaceStore } from '@/stores/workspace';
import {
  useNotifyDefault,
  useSetNotifyDefault,
  type NotifyLevel,
} from '@/features/notifications/hooks/useNotifyLevels';

export function NotifyDefaultPanel() {
  const { t } = useTranslation('notifications');
  const workspace = useWorkspaceStore((s) => s.currentWorkspace);
  const { data: level } = useNotifyDefault(workspace?.id ?? null, workspace?.instanceUrl);
  const setDefault = useSetNotifyDefault(workspace?.id ?? '', workspace?.instanceUrl);
  if (!workspace || !level) return null;

  return (
    <div data-qa="notify-default">
      <RadioCards<NotifyLevel>
        label={t('default.label', { name: workspace.name })}
        value={level}
        columns={1}
        onChange={(next) =>
          setDefault.mutate(next, {
            onError: () => toast.error(t('default.saveFailed')),
          })
        }
        options={(['all', 'mentions', 'nothing'] as const).map((value) => ({
          value,
          label: t(`levels.${value}`),
          description: t(`default.descriptions.${value}`),
          dataQa: `notify-default-${value}`,
        }))}
      />
      <p className="mt-2 text-xs text-muted">{t('default.override')}</p>
    </div>
  );
}
