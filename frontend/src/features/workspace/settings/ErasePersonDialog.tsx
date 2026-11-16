import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Trans } from '@/shared/components/Trans/Trans';
import { ConfirmDialog } from '@/shared/components/ConfirmDialog/ConfirmDialog';
import { TextInput } from '@/shared/components/Form/TextInput';

interface Props {
  name: string;
  email: string;
  onErase: () => Promise<unknown>;
  onClose: () => void;
}

export function ErasePersonDialog({ name, email, onErase, onClose }: Props) {
  const { t } = useTranslation('workspace');
  const [typed, setTyped] = useState('');
  const matches = typed.trim() === name.trim();

  return (
    <ConfirmDialog
      title={t('settings.erase.title', { name })}
      description={t('settings.erase.description')}
      confirmLabel={t('settings.erase.confirm')}
      confirmDisabled={!matches}
      onConfirm={async () => {
        await onErase();
      }}
      onClose={onClose}
      dataQa="erase-person-dialog"
    >
      <ul className="mb-4 list-disc space-y-1 pl-5 text-sm text-fg-soft" data-qa="erase-person-consequences">
        <li>
          <Trans
            t={t}
            i18nKey="settings.erase.replaced"
            values={{ email: email || t('settings.erase.noEmail') }}
            components={{ email: <span data-qa="erase-person-email" />, em: <em /> }}
          />
        </li>
        <li>{t('settings.erase.signedOut')}</li>
        <li>{t('settings.erase.apps')}</li>
        <li>{t('settings.erase.deleted')}</li>
        <li>{t('settings.erase.messages')}</li>
      </ul>
      <TextInput
        label={t('settings.erase.typeName', { name })}
        value={typed}
        onChange={(e) => setTyped(e.target.value)}
        autoComplete="off"
        dataQa="erase-person-name"
      />
    </ConfirmDialog>
  );
}
