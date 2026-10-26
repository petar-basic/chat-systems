import { useState } from 'react';
import { ConfirmDialog } from '@/shared/components/ConfirmDialog/ConfirmDialog';
import { TextInput } from '@/shared/components/Form/TextInput';

interface Props {
  name: string;
  email: string;
  onErase: () => Promise<unknown>;
  onClose: () => void;
}

export function ErasePersonDialog({ name, email, onErase, onClose }: Props) {
  const [typed, setTyped] = useState('');
  const matches = typed.trim() === name.trim();

  return (
    <ConfirmDialog
      title={`Erase ${name}'s data?`}
      description="This applies to every workspace on this instance and cannot be undone."
      confirmLabel="Erase data"
      confirmDisabled={!matches}
      onConfirm={async () => {
        await onErase();
      }}
      onClose={onClose}
      dataQa="erase-person-dialog"
    >
      <ul className="mb-4 list-disc space-y-1 pl-5 text-sm text-fg-soft" data-qa="erase-person-consequences">
        <li>
          Their name, email ({email || 'none'}), avatar and bio are replaced with <em>Deleted user</em>.
        </li>
        <li>They are signed out everywhere and can no longer sign in.</li>
        <li>Their connected apps are disconnected.</li>
        <li>Their messages stay readable, shown as from Deleted user.</li>
      </ul>
      <TextInput
        label={`Type ${name} to confirm`}
        value={typed}
        onChange={(e) => setTyped(e.target.value)}
        autoComplete="off"
        dataQa="erase-person-name"
      />
    </ConfirmDialog>
  );
}
