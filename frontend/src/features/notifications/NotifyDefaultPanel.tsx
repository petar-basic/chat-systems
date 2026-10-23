import { RadioCards } from '@/shared/components/Form/RadioCards';
import { toast } from '@/shared/components/Toast';
import { useWorkspaceStore } from '@/stores/workspace';
import { useNotifyDefault, useSetNotifyDefault, type NotifyLevel } from '@/hooks/queries/useNotifyLevels';
import { NOTIFY_LEVEL_LABELS } from './notifyLevels';

const DESCRIPTIONS: Record<NotifyLevel, string> = {
  all: 'Every new message in your channels, plus mentions, replies and direct messages.',
  mentions: 'Direct messages, mentions and replies in threads you follow.',
  nothing: 'Only direct messages. Channels stay quiet until you open them.',
};

export function NotifyDefaultPanel() {
  const workspace = useWorkspaceStore((s) => s.currentWorkspace);
  const { data: level } = useNotifyDefault(workspace?.id ?? null, workspace?.instanceUrl);
  const setDefault = useSetNotifyDefault(workspace?.id ?? '', workspace?.instanceUrl);
  if (!workspace || !level) return null;

  return (
    <div data-qa="notify-default">
      <RadioCards<NotifyLevel>
        label={`Notify me about, in ${workspace.name}`}
        value={level}
        columns={1}
        onChange={(next) =>
          setDefault.mutate(next, {
            onError: () => toast.error("Couldn't change your notification default. Try again."),
          })
        }
        options={(['all', 'mentions', 'nothing'] as const).map((value) => ({
          value,
          label: NOTIFY_LEVEL_LABELS[value],
          description: DESCRIPTIONS[value],
          dataQa: `notify-default-${value}`,
        }))}
      />
      <p className="mt-2 text-xs text-muted">
        A channel can override this from its bell menu. Direct messages notify you unless you change them one
        by one.
      </p>
    </div>
  );
}
