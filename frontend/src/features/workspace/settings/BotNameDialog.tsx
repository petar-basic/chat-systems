import { useState, type FormEvent } from 'react';
import { useTranslation } from 'react-i18next';
import { Modal } from '@/shared/components/Modal/Modal';
import { Button } from '@/shared/components/Button/Button';
import { TextInput } from '@/shared/components/Form/TextInput';
import { useRenameResponsesBot } from '@/features/custom-responses';
import { toUserMessage } from '@/lib/errors';

interface Props {
  workspaceId: string;
  instanceUrl?: string;
  name: string;
  onClose: () => void;
}

export function BotNameDialog({ workspaceId, instanceUrl, name, onClose }: Props) {
  const { t } = useTranslation(['workspace', 'common']);
  const rename = useRenameResponsesBot(workspaceId, instanceUrl);
  const [draft, setDraft] = useState(name);
  const [error, setError] = useState<string | null>(null);
  const ready = draft.trim().length > 0 && draft.trim() !== name;

  const submit = (e: FormEvent) => {
    e.preventDefault();
    if (!ready) return;
    setError(null);
    rename.mutate(draft.trim(), {
      onSuccess: onClose,
      onError: (err) => setError(toUserMessage(err, t('customResponses.renameFailed'))),
    });
  };

  return (
    <Modal
      title={t('customResponses.renameTitle')}
      onClose={onClose}
      onSubmit={submit}
      dataQa="custom-responses-rename-dialog"
      dismissLabel={t('cancel', { ns: 'common' })}
      actions={
        <Button
          type="submit"
          variant="primary"
          loading={rename.isPending}
          disabled={!ready}
          data-qa="custom-responses-rename-save"
        >
          {t('customResponses.renameSave')}
        </Button>
      }
    >
      <div className="space-y-3">
        <TextInput
          label={t('customResponses.botName')}
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          maxLength={100}
          autoComplete="off"
          dataQa="custom-responses-bot-name-input"
          data-autofocus
        />
        {error && (
          <p role="alert" className="text-sm text-danger" data-qa="custom-responses-rename-error">
            {error}
          </p>
        )}
      </div>
    </Modal>
  );
}
