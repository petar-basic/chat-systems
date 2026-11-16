import { useState, type DragEvent, type KeyboardEvent } from 'react';
import { useTranslation } from 'react-i18next';
import { GripVertical, Pencil, Plus, Trash2 } from 'lucide-react';
import { Button } from '@/shared/components/Button/Button';
import { IconButton } from '@/shared/components/IconButton/IconButton';
import { ConfirmDialog } from '@/shared/components/ConfirmDialog/ConfirmDialog';
import { Spinner } from '@/shared/components/Spinner/Spinner';
import { toast } from '@/shared/components/Toast';
import {
  MAX_FIELDS,
  MAX_ON_CARD,
  moveItem,
  useDeleteProfileField,
  useOrderProfileFields,
  useProfileFields,
  type ProfileField,
} from '@/features/people';
import { toUserMessage } from '@/lib/errors';
import { ProfileFieldDialog } from './ProfileFieldDialog';

interface Props {
  workspaceId: string;
  instanceUrl?: string;
}

export function ProfileFieldsSection({ workspaceId, instanceUrl }: Props) {
  const { t } = useTranslation(['workspace', 'common']);
  const { fields, isLoading } = useProfileFields(workspaceId, instanceUrl);
  const order = useOrderProfileFields(workspaceId, instanceUrl);
  const remove = useDeleteProfileField(workspaceId, instanceUrl);
  const [editing, setEditing] = useState<ProfileField | 'new' | null>(null);
  const [deleting, setDeleting] = useState<ProfileField | null>(null);
  const [dragging, setDragging] = useState<number | null>(null);
  const onCard = fields.filter((f) => f.on_card).length;

  const reorder = (from: number, to: number) => {
    const next = moveItem(fields, from, to);
    if (next === fields) return;
    order.mutate(
      next.map((f) => f.id),
      { onError: (err) => toast.error(toUserMessage(err, t('profileFields.orderFailed'))) },
    );
  };

  const onGripKey = (index: number) => (e: KeyboardEvent<HTMLButtonElement>) => {
    if (e.key !== 'ArrowUp' && e.key !== 'ArrowDown') return;
    e.preventDefault();
    reorder(index, index + (e.key === 'ArrowUp' ? -1 : 1));
  };

  const dropOn = (index: number) => (e: DragEvent<HTMLLIElement>) => {
    e.preventDefault();
    if (dragging !== null) reorder(dragging, index);
    setDragging(null);
  };

  const confirmDelete = async () => {
    if (!deleting) return;
    try {
      await remove.mutateAsync(deleting.id);
      setDeleting(null);
    } catch (err) {
      toast.error(toUserMessage(err, t('profileFields.deleteFailed')));
    }
  };

  return (
    <div className="space-y-4" data-qa="profile-fields-settings">
      <div className="flex items-start justify-between gap-4">
        <p className="text-sm text-fg-dim max-w-prose">{t('profileFields.explain')}</p>
        <Button
          variant="primary"
          icon={<Plus className="h-4 w-4" />}
          onClick={() => setEditing('new')}
          disabled={fields.length >= MAX_FIELDS}
          data-qa="profile-fields-add"
        >
          {t('profileFields.add')}
        </Button>
      </div>

      {isLoading ? (
        <div className="flex justify-center py-8 text-accent">
          <Spinner />
        </div>
      ) : fields.length === 0 ? (
        <p className="text-sm text-muted py-6 text-center" data-qa="profile-fields-empty">
          {t('profileFields.empty')}
        </p>
      ) : (
        <ol
          className="divide-y divide-line/50 rounded-lg border border-line/60"
          data-qa="profile-fields-list"
        >
          {fields.map((field, index) => (
            <li
              key={field.id}
              draggable
              onDragStart={(e) => {
                e.dataTransfer.effectAllowed = 'move';
                e.dataTransfer.setData('text/plain', field.id);
                setDragging(index);
              }}
              onDragEnd={() => setDragging(null)}
              onDragOver={(e) => e.preventDefault()}
              onDrop={dropOn(index)}
              data-qa="profile-fields-row"
              className={`flex items-center gap-2 px-3 py-2.5 ${dragging === index ? 'opacity-50' : ''}`}
            >
              <IconButton
                label={t('profileFields.move', { label: field.label })}
                icon={<GripVertical className="h-4 w-4" />}
                onKeyDown={onGripKey(index)}
                className="cursor-grab"
              />
              <div className="min-w-0 flex-1">
                <div className="flex flex-wrap items-center gap-1.5">
                  <span className="text-sm font-medium text-fg truncate">{field.label}</span>
                  <span className="rounded bg-raised px-1.5 py-0.5 text-2xs text-fg-dim">
                    {t(`profileFields.kinds.${field.kind}`)}
                  </span>
                  {field.admins_only && (
                    <span className="rounded bg-raised px-1.5 py-0.5 text-2xs text-fg-dim">
                      {t('profileFields.adminsOnlyBadge')}
                    </span>
                  )}
                  {field.on_card && (
                    <span className="rounded bg-accent/15 px-1.5 py-0.5 text-2xs text-accent">
                      {t('profileFields.onCardBadge')}
                    </span>
                  )}
                </div>
                <div className="text-xs text-muted truncate">
                  {field.kind === 'options'
                    ? field.options.map((o) => o.label).join(' · ')
                    : (field.hint ?? '')}
                </div>
              </div>
              <IconButton
                label={t('profileFields.editNamed', { label: field.label })}
                icon={<Pencil className="h-4 w-4" />}
                onClick={() => setEditing(field)}
                data-qa="profile-fields-edit"
              />
              <IconButton
                label={t('profileFields.deleteNamed', { label: field.label })}
                icon={<Trash2 className="h-4 w-4" />}
                tone="danger"
                onClick={() => setDeleting(field)}
                data-qa="profile-fields-delete"
              />
            </li>
          ))}
        </ol>
      )}
      {fields.length > 1 && <p className="text-xs text-muted">{t('profileFields.reorderHint')}</p>}

      {editing && (
        <ProfileFieldDialog
          workspaceId={workspaceId}
          instanceUrl={instanceUrl}
          field={editing === 'new' ? null : editing}
          cardFull={onCard >= MAX_ON_CARD}
          onClose={() => setEditing(null)}
        />
      )}
      {deleting && (
        <ConfirmDialog
          title={t('profileFields.deleteTitle', { label: deleting.label })}
          description={t('profileFields.deleteDescription', { count: deleting.value_count })}
          confirmLabel={t('profileFields.delete')}
          tone="danger"
          onConfirm={confirmDelete}
          onClose={() => setDeleting(null)}
          dataQa="profile-fields-delete-dialog"
        />
      )}
    </div>
  );
}
