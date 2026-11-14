import { useState, type FormEvent } from 'react';
import { useTranslation } from 'react-i18next';
import { Modal } from '@/shared/components/Modal/Modal';
import { Button } from '@/shared/components/Button/Button';
import {
  ProfileFieldInputs,
  changedValues,
  useSetProfileValues,
  type FieldValues,
  type ProfileField,
} from '@/features/people';
import { toUserMessage } from '@/lib/errors';

interface Props {
  userId: string;
  name: string;
  workspaceId: string;
  instanceUrl?: string;
  fields: ProfileField[];
  values: FieldValues;
  onClose: () => void;
}

export function ProfileValuesDialog({
  userId,
  name,
  workspaceId,
  instanceUrl,
  fields,
  values,
  onClose,
}: Props) {
  const { t } = useTranslation(['profile', 'common']);
  const save = useSetProfileValues(workspaceId, instanceUrl);
  const [draft, setDraft] = useState<FieldValues>(values);
  const [error, setError] = useState<string | null>(null);

  const submit = (e: FormEvent) => {
    e.preventDefault();
    const changes = changedValues(values, draft, fields);
    if (Object.keys(changes).length === 0) {
      onClose();
      return;
    }
    setError(null);
    save.mutate(
      { userId, values: changes },
      {
        onSuccess: onClose,
        onError: (err) => setError(toUserMessage(err, t('fields.saveFailed'))),
      },
    );
  };

  return (
    <Modal
      title={t('panel.editFieldsOf', { name })}
      onClose={onClose}
      onSubmit={submit}
      dataQa="profile-values-dialog"
      dismissLabel={t('cancel', { ns: 'common' })}
      actions={
        <Button type="submit" variant="primary" loading={save.isPending} data-qa="profile-values-save">
          {t('fields.save')}
        </Button>
      }
    >
      <div className="space-y-4">
        <ProfileFieldInputs
          fields={fields}
          values={draft}
          onChange={(fieldId, value) => setDraft((d) => ({ ...d, [fieldId]: value }))}
        />
        {error && (
          <p role="alert" className="text-sm text-danger">
            {error}
          </p>
        )}
      </div>
    </Modal>
  );
}
