import { RadioCards } from '@/shared/components/Form/RadioCards';
import { useInstanceSettings, useUpdateInstanceSettings } from '@/features/instance';
import { toUserMessage } from '@/lib/errors';

interface Props {
  instanceUrl: string | null;
}

type Who = 'everyone' | 'creators';

export function WorkspaceCreationSetting({ instanceUrl }: Props) {
  const { data } = useInstanceSettings(instanceUrl);
  const update = useUpdateInstanceSettings(instanceUrl);
  if (!data) return null;

  const who: Who = data.workspace_creation_restricted ? 'creators' : 'everyone';

  return (
    <div className="space-y-2" data-qa="workspace-creation-setting">
      <RadioCards<Who>
        label="Who can create workspaces"
        value={who}
        onChange={(next) => {
          if (next !== who && !update.isPending)
            update.mutate({ workspace_creation_restricted: next === 'creators' });
        }}
        options={[
          {
            value: 'everyone',
            label: 'Everyone',
            description: 'Anyone who can sign in can start a workspace.',
            dataQa: 'workspace-creation-everyone',
          },
          {
            value: 'creators',
            label: 'Workspace creators and admins',
            description:
              'Give the Workspace creator role to the people who start workspaces. Everyone else joins the ones they are invited to.',
            dataQa: 'workspace-creation-creators',
          },
        ]}
      />
      {update.isError && (
        <p className="text-sm text-danger" role="alert">
          {toUserMessage(update.error, 'Could not save the setting')}
        </p>
      )}
    </div>
  );
}
