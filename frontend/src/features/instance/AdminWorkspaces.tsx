import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Crown } from 'lucide-react';
import { Spinner } from '@/shared/components/Spinner/Spinner';
import { ConfirmDialog } from '@/shared/components/ConfirmDialog/ConfirmDialog';
import { Select } from '@/shared/components/Form/Select';
import { toUserMessage } from '@/lib/errors';
import {
  useAdminWorkspacePeople,
  useAdminWorkspaces,
  useAppointOwner,
  type AdminWorkspace,
  type WorkspacePerson,
} from './hooks/useAdmin';

const nameOf = (person: WorkspacePerson) => person.display_name || person.email;

function AppointOwnerDialog({
  workspace,
  instanceUrl,
  onClose,
}: {
  workspace: AdminWorkspace;
  instanceUrl: string | null;
  onClose: () => void;
}) {
  const { t } = useTranslation(['admin', 'common']);
  const { data: people, isLoading } = useAdminWorkspacePeople(instanceUrl, workspace.id);
  const appoint = useAppointOwner(instanceUrl);
  const candidates = (people ?? []).filter((p) => p.role !== 'owner' && p.status === 'active');
  const [chosen, setChosen] = useState('');
  const userId = chosen || candidates[0]?.user_id || '';

  const confirm = async () => {
    try {
      await appoint.mutateAsync({ workspaceId: workspace.id, userId });
    } catch (err) {
      throw new Error(toUserMessage(err, t('workspaces.appointFailed')), { cause: err });
    }
  };

  return (
    <ConfirmDialog
      title={t('workspaces.appointTitle', { name: workspace.name })}
      description={t('workspaces.appointDescription')}
      confirmLabel={t('workspaces.appointConfirm')}
      tone="primary"
      confirmDisabled={!userId}
      onConfirm={confirm}
      onClose={onClose}
      dataQa="appoint-owner-dialog"
    >
      {isLoading ? (
        <Spinner className="text-accent" />
      ) : candidates.length === 0 ? (
        <p className="text-sm text-muted">{t('workspaces.noCandidates')}</p>
      ) : (
        <Select
          label={t('workspaces.appointMember')}
          value={userId}
          onChange={(e) => setChosen(e.target.value)}
          dataQa="appoint-owner-select"
        >
          {candidates.map((p) => (
            <option key={p.user_id} value={p.user_id}>
              {p.display_name ? `${p.display_name} (${p.email})` : p.email} ·{' '}
              {t(`common:workspaceRoles.${p.role}`)}
            </option>
          ))}
        </Select>
      )}
    </ConfirmDialog>
  );
}

export function AdminWorkspaces({ instanceUrl }: { instanceUrl: string | null }) {
  const { t } = useTranslation(['admin', 'common']);
  const { data: workspaces = [], isLoading, error } = useAdminWorkspaces(instanceUrl);
  const [appointing, setAppointing] = useState<AdminWorkspace | null>(null);

  if (isLoading) {
    return (
      <div className="flex items-center justify-center py-20">
        <Spinner size="lg" className="text-accent" />
      </div>
    );
  }

  if (error) {
    return (
      <div className="max-w-4xl mx-auto bg-red-500/10 border border-red-500/30 text-danger px-4 py-3 rounded-lg text-sm">
        {toUserMessage(error)}
      </div>
    );
  }

  return (
    <div className="max-w-4xl mx-auto">
      <div className="mb-4 text-sm text-muted">{t('workspaces.count', { count: workspaces.length })}</div>
      <div className="bg-surface rounded-xl border border-line overflow-x-auto">
        <table className="w-full min-w-140">
          <thead>
            <tr className="border-b border-line">
              <th className="text-left px-4 py-3 text-xs font-medium text-muted uppercase tracking-wider">
                {t('workspaces.workspace')}
              </th>
              <th className="text-left px-4 py-3 text-xs font-medium text-muted uppercase tracking-wider">
                {t('workspaces.owners')}
              </th>
              <th className="text-right px-4 py-3 text-xs font-medium text-muted uppercase tracking-wider">
                {t('common:actions')}
              </th>
            </tr>
          </thead>
          <tbody className="divide-y divide-line">
            {workspaces.map((workspace) => {
              const stranded = !workspace.owners.some((o) => o.status === 'active');
              return (
                <tr
                  key={workspace.id}
                  className="hover:bg-raised/30 transition"
                  data-qa="admin-workspace-row"
                >
                  <td className="px-4 py-3">
                    <div className="font-medium text-sm text-fg">{workspace.name}</div>
                    <div className="text-xs text-muted">
                      {workspace.slug}
                      {!workspace.is_active && (
                        <span className="ml-2 text-danger">{t('workspaces.deleted')}</span>
                      )}
                    </div>
                  </td>
                  <td className="px-4 py-3">
                    <ul className="space-y-0.5">
                      {workspace.owners.map((owner) => (
                        <li key={owner.user_id} className="flex items-center gap-1.5 text-sm text-fg-soft">
                          <Crown className="w-3.5 h-3.5 text-warning shrink-0" />
                          <span className="truncate">{nameOf(owner)}</span>
                          {owner.status !== 'active' && (
                            <span className="px-1.5 py-0.5 rounded text-xs bg-red-500/10 text-danger">
                              {t(`common:userStatus.${owner.status}`)}
                            </span>
                          )}
                        </li>
                      ))}
                    </ul>
                    {stranded && (
                      <div className="mt-1 text-xs text-danger" data-qa="admin-workspace-stranded">
                        {t('workspaces.stranded')}
                      </div>
                    )}
                  </td>
                  <td className="px-4 py-3 text-right">
                    <button
                      onClick={() => setAppointing(workspace)}
                      data-qa="admin-workspace-appoint"
                      className="px-2 py-1.5 rounded text-xs text-muted hover:text-fg hover:bg-raised transition cursor-pointer"
                    >
                      {t('workspaces.appoint')}
                    </button>
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
      {appointing && (
        <AppointOwnerDialog
          workspace={appointing}
          instanceUrl={instanceUrl}
          onClose={() => setAppointing(null)}
        />
      )}
    </div>
  );
}
