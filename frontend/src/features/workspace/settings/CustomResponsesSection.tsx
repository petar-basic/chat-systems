import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Pencil, Plus, Trash2 } from 'lucide-react';
import { Avatar } from '@/shared/components/Avatar/Avatar';
import { Button } from '@/shared/components/Button/Button';
import { IconButton } from '@/shared/components/IconButton/IconButton';
import { ConfirmDialog } from '@/shared/components/ConfirmDialog/ConfirmDialog';
import { QueryState } from '@/shared/components/QueryState/QueryState';
import { toast } from '@/shared/components/Toast';
import {
  useCustomResponses,
  useDeleteCustomResponse,
  type CustomResponse,
} from '@/features/custom-responses';
import { toUserMessage } from '@/lib/errors';
import { CustomResponseDialog } from './CustomResponseDialog';
import { BotNameDialog } from './BotNameDialog';

interface Props {
  workspaceId: string;
  instanceUrl?: string;
}

export function CustomResponsesSection({ workspaceId, instanceUrl }: Props) {
  const { t } = useTranslation(['workspace', 'common']);
  const query = useCustomResponses(workspaceId, instanceUrl);
  const remove = useDeleteCustomResponse(workspaceId, instanceUrl);
  const [editing, setEditing] = useState<CustomResponse | 'new' | null>(null);
  const [deleting, setDeleting] = useState<CustomResponse | null>(null);
  const [renaming, setRenaming] = useState(false);
  const page = query.data;
  const botName = page?.bot.name ?? '';

  const confirmDelete = async () => {
    if (!deleting) return;
    try {
      await remove.mutateAsync(deleting.id);
      setDeleting(null);
    } catch (err) {
      toast.error(toUserMessage(err, t('customResponses.deleteFailed')));
    }
  };

  return (
    <QueryState
      isLoading={query.isLoading}
      isError={query.isError}
      onRetry={() => void query.refetch()}
      errorMessage={toUserMessage(query.error, t('customResponses.loadFailed'))}
    >
      {page && (
        <div className="space-y-4" data-qa="custom-responses-settings">
          <div className="flex items-start justify-between gap-4">
            <p className="text-sm text-fg-dim max-w-prose">
              {t('customResponses.explain', { name: botName })}
            </p>
            {page.can_edit && (
              <Button
                variant="primary"
                icon={<Plus className="h-4 w-4" />}
                onClick={() => setEditing('new')}
                data-qa="custom-responses-add"
              >
                {t('customResponses.add')}
              </Button>
            )}
          </div>

          <div
            className="flex items-center gap-3 rounded-lg border border-line/60 px-3 py-2.5"
            data-qa="custom-responses-bot"
          >
            <Avatar userId={page.bot.user_id ?? workspaceId} name={botName} avatarUrl={page.bot.avatar_url} />
            <div className="min-w-0 flex-1">
              <div className="text-xs text-muted">{t('customResponses.bot')}</div>
              <div className="flex items-center gap-1.5">
                <span className="text-sm font-medium text-fg truncate" data-qa="custom-responses-bot-name">
                  {botName}
                </span>
                <span className="px-1 py-0.5 text-3xs font-semibold uppercase tracking-wide bg-raised text-fg-dim rounded">
                  {t('customResponses.app')}
                </span>
              </div>
            </div>
            {page.can_rename && (
              <Button
                variant="ghost"
                size="sm"
                onClick={() => setRenaming(true)}
                data-qa="custom-responses-rename"
              >
                {t('customResponses.rename')}
              </Button>
            )}
          </div>

          {!page.can_edit && (
            <p className="text-sm text-muted" data-qa="custom-responses-read-only">
              {t('customResponses.readOnly')}
            </p>
          )}

          {page.data.length === 0 ? (
            <p className="text-sm text-muted py-6 text-center" data-qa="custom-responses-empty">
              {t('customResponses.empty')}
            </p>
          ) : (
            <ul
              className="divide-y divide-line/50 rounded-lg border border-line/60"
              data-qa="custom-responses-list"
            >
              {page.data.map((response) => (
                <li
                  key={response.id}
                  className="flex items-start gap-2 px-3 py-2.5"
                  data-qa="custom-responses-row"
                >
                  <div className="min-w-0 flex-1">
                    <div className="flex flex-wrap gap-1.5">
                      {response.triggers.map((trigger) => (
                        <span
                          key={trigger}
                          className="rounded bg-raised px-1.5 py-0.5 text-xs font-medium text-fg"
                          data-qa="custom-responses-trigger"
                        >
                          {trigger}
                        </span>
                      ))}
                    </div>
                    <p className="mt-1 text-xs text-muted truncate">
                      {response.answers[0]}
                      {response.answers.length > 1 &&
                        ` · ${t('customResponses.moreAnswers', { count: response.answers.length - 1 })}`}
                    </p>
                  </div>
                  {page.can_edit && (
                    <>
                      <IconButton
                        label={t('customResponses.editNamed', { phrase: response.triggers[0] })}
                        icon={<Pencil className="h-4 w-4" />}
                        onClick={() => setEditing(response)}
                        data-qa="custom-responses-edit"
                      />
                      <IconButton
                        label={t('customResponses.deleteNamed', { phrase: response.triggers[0] })}
                        icon={<Trash2 className="h-4 w-4" />}
                        tone="danger"
                        onClick={() => setDeleting(response)}
                        data-qa="custom-responses-delete"
                      />
                    </>
                  )}
                </li>
              ))}
            </ul>
          )}

          {editing && (
            <CustomResponseDialog
              workspaceId={workspaceId}
              instanceUrl={instanceUrl}
              botName={botName}
              response={editing === 'new' ? null : editing}
              onClose={() => setEditing(null)}
            />
          )}
          {deleting && (
            <ConfirmDialog
              title={t('customResponses.deleteTitle', { phrase: deleting.triggers[0] })}
              description={t('customResponses.deleteDescription', { name: botName })}
              confirmLabel={t('customResponses.delete')}
              tone="danger"
              onConfirm={confirmDelete}
              onClose={() => setDeleting(null)}
              dataQa="custom-responses-delete-dialog"
            />
          )}
          {renaming && (
            <BotNameDialog
              workspaceId={workspaceId}
              instanceUrl={instanceUrl}
              name={botName}
              onClose={() => setRenaming(false)}
            />
          )}
        </div>
      )}
    </QueryState>
  );
}
