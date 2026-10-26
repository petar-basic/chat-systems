import { useState, type ChangeEvent, type FormEvent } from 'react';
import { Button } from '@/shared/components/Button/Button';
import { TextInput } from '@/shared/components/Form/TextInput';
import { useCompleteSetup, type SetupDraft } from '@/features/instance';
import { toUserMessage } from '@/lib/errors';

interface Props {
  onDone: (workspaceId: string) => void;
}

const EMPTY: SetupDraft = { token: '', displayName: '', email: '', password: '', workspaceName: '' };

export function SetupForm({ onDone }: Props) {
  const complete = useCompleteSetup();
  const [draft, setDraft] = useState(EMPTY);

  const field = (key: keyof SetupDraft) => (e: ChangeEvent<HTMLInputElement>) =>
    setDraft((d) => ({ ...d, [key]: e.target.value }));

  const tooShort = draft.password.length > 0 && draft.password.length < 8;

  const submit = (e: FormEvent) => {
    e.preventDefault();
    if (tooShort) return;
    complete.mutate(draft, { onSuccess: (res) => onDone(res.workspace_id) });
  };

  return (
    <form onSubmit={submit} className="space-y-5" data-qa="setup-form">
      {complete.isError && (
        <div
          role="alert"
          className="bg-red-500/10 border border-red-500/30 text-danger px-4 py-3 rounded-lg text-sm"
          data-qa="setup-error"
        >
          {toUserMessage(complete.error, 'Setup failed')}
        </div>
      )}

      <TextInput
        label="Setup token"
        hint="The API prints it in its log on every start until setup is done."
        value={draft.token}
        onChange={field('token')}
        autoComplete="off"
        spellCheck={false}
        dataQa="setup-token"
        required
      />
      <TextInput
        label="Your name"
        value={draft.displayName}
        onChange={field('displayName')}
        autoComplete="name"
        dataQa="setup-name"
        required
      />
      <TextInput
        label="Email"
        type="email"
        value={draft.email}
        onChange={field('email')}
        autoComplete="email"
        dataQa="setup-email"
        required
      />
      <TextInput
        label="Password"
        type="password"
        value={draft.password}
        onChange={field('password')}
        autoComplete="new-password"
        error={tooShort ? 'At least 8 characters' : undefined}
        dataQa="setup-password"
        required
      />
      <TextInput
        label="Workspace name"
        hint="Your team's first workspace. You can add more later."
        value={draft.workspaceName}
        onChange={field('workspaceName')}
        placeholder="Acme"
        dataQa="setup-workspace"
        required
      />

      <Button variant="primary" fullWidth type="submit" loading={complete.isPending} data-qa="setup-submit">
        Set up this instance
      </Button>
    </form>
  );
}
