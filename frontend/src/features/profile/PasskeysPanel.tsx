import { useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Fingerprint, Pencil, Trash2 } from 'lucide-react';
import { api } from '@/lib/api';
import { toUserMessage } from '@/lib/errors';
import { formatDateTime } from '@/lib/datetime';
import { desktopShell } from '@/lib/desktopShell';
import { createPasskey, deviceName, passkeysSupported, wasCancelled } from '@/lib/webauthn';
import { Button } from '@/shared/components/Button/Button';
import { TextInput } from '@/shared/components/Form/TextInput';
import { toast } from '@/shared/components/Toast';

const MAX_PASSKEYS = 10;

type Pending = { kind: 'add' } | { kind: 'remove'; id: string; name: string };

function proofOf(value: string) {
  return /^\d{6}$/.test(value) ? { code: value } : { password: value };
}

export default function PasskeysPanel() {
  const queryClient = useQueryClient();
  const [pending, setPending] = useState<Pending | null>(null);
  const [proof, setProof] = useState('');
  const [name, setName] = useState(() => deviceName());
  const [renaming, setRenaming] = useState<{ id: string; name: string } | null>(null);
  const [error, setError] = useState<string | null>(null);

  const { data } = useQuery({
    queryKey: ['passkeys'],
    queryFn: () => api.typed((c) => c.GET('/auth/passkeys')),
  });

  const refresh = () => queryClient.invalidateQueries({ queryKey: ['passkeys'] });
  const close = () => {
    setPending(null);
    setProof('');
    setError(null);
  };
  const failed = (e: unknown) => setError(wasCancelled(e) ? null : toUserMessage(e));

  const add = useMutation({
    mutationFn: async () => {
      const challenge = await api.typed((c) =>
        c.POST('/auth/passkeys/registration', { body: proofOf(proof) }),
      );
      const credential = await createPasskey(challenge.options);
      return api.typed((c) =>
        c.POST('/auth/passkeys', {
          body: { challenge_id: challenge.challenge_id, name: name.trim(), credential },
        }),
      );
    },
    onSuccess: () => {
      close();
      setName(deviceName());
      toast.success('Passkey added.');
      void refresh();
    },
    onError: failed,
  });

  const remove = useMutation({
    mutationFn: (id: string) =>
      api.typed((c) =>
        c.POST('/auth/passkeys/{id}/remove', { params: { path: { id } }, body: proofOf(proof) }),
      ),
    onSuccess: () => {
      close();
      toast.success('Passkey removed.');
      void refresh();
    },
    onError: failed,
  });

  const rename = useMutation({
    mutationFn: ({ id, name }: { id: string; name: string }) =>
      api.typed((c) =>
        c.PATCH('/auth/passkeys/{id}', { params: { path: { id } }, body: { name: name.trim() } }),
      ),
    onSuccess: () => {
      setRenaming(null);
      setError(null);
      void refresh();
    },
    onError: failed,
  });

  if (!data) return null;
  if (!data.offered) {
    return (
      <div data-qa="passkeys" className="mt-8">
        <PanelHeading />
        <p className="text-xs text-muted mt-1">
          Your account signs in through single sign-on, so your identity provider is where a passkey belongs.
        </p>
      </div>
    );
  }

  const inDesktop = desktopShell() !== null;
  const canAdd = !inDesktop && passkeysSupported() && data.data.length < MAX_PASSKEYS;
  const busy = add.isPending || remove.isPending;

  return (
    <div data-qa="passkeys" className="mt-8">
      <div className="flex items-start justify-between gap-4 mb-3">
        <div>
          <PanelHeading />
          <p className="text-xs text-muted mt-1">
            {inDesktop
              ? 'Add a passkey from the browser. The ones you have are listed here.'
              : 'Sign in with your fingerprint, face or device PIN instead of a password and code.'}
          </p>
        </div>
        {canAdd && !pending && (
          <Button variant="primary" onClick={() => setPending({ kind: 'add' })} data-qa="passkey-add">
            Add a passkey
          </Button>
        )}
      </div>

      {error && (
        <div className="text-sm text-danger bg-red-500/10 border border-red-500/30 rounded-lg px-3 py-2 mb-3">
          {error}
        </div>
      )}

      {pending && (
        <div className="space-y-3 mb-4 rounded-lg border border-line p-3" data-qa="passkey-confirm">
          <p className="text-xs text-muted">
            {pending.kind === 'add'
              ? 'Confirm it is you with your password or a code from your authenticator app.'
              : `Removing “${pending.name}”. Confirm with your password or a code from your authenticator app.`}
          </p>
          {pending.kind === 'add' && (
            <TextInput
              label="Name"
              value={name}
              onChange={(e) => setName(e.target.value)}
              maxLength={64}
              dataQa="passkey-name"
            />
          )}
          <TextInput
            label="Password or code"
            type="password"
            autoComplete="current-password"
            value={proof}
            onChange={(e) => setProof(e.target.value)}
            dataQa="passkey-proof"
          />
          <div className="flex gap-2">
            <Button
              variant={pending.kind === 'add' ? 'primary' : 'danger'}
              onClick={() => (pending.kind === 'add' ? add.mutate() : remove.mutate(pending.id))}
              disabled={busy || !proof || (pending.kind === 'add' && !name.trim())}
              loading={busy}
              data-qa="passkey-confirm-submit"
            >
              {pending.kind === 'add' ? 'Continue' : 'Remove'}
            </Button>
            <Button variant="ghost" onClick={close}>
              Cancel
            </Button>
          </div>
        </div>
      )}

      {data.data.length === 0 ? (
        <p className="text-sm text-muted" data-qa="passkeys-empty">
          No passkeys yet.
        </p>
      ) : (
        <ul className="divide-y divide-line rounded-lg border border-line" data-qa="passkey-list">
          {data.data.map((passkey) => (
            <li key={passkey.id} className="flex items-center gap-3 px-3 py-2.5" data-qa="passkey-item">
              {renaming?.id === passkey.id ? (
                <div className="flex flex-1 gap-2">
                  <TextInput
                    label="Passkey name"
                    labelHidden
                    value={renaming.name}
                    onChange={(e) => setRenaming({ id: passkey.id, name: e.target.value })}
                    maxLength={64}
                    autoFocus
                    dataQa="passkey-rename-input"
                    className="flex-1"
                  />
                  <Button
                    variant="primary"
                    onClick={() => rename.mutate(renaming)}
                    disabled={rename.isPending || !renaming.name.trim()}
                    data-qa="passkey-rename-save"
                  >
                    Save
                  </Button>
                  <Button variant="ghost" onClick={() => setRenaming(null)}>
                    Cancel
                  </Button>
                </div>
              ) : (
                <>
                  <div className="min-w-0 flex-1">
                    <p className="truncate text-sm text-fg">{passkey.name}</p>
                    <p className="text-xs text-muted">
                      Added {formatDateTime(passkey.created_at)} ·{' '}
                      {passkey.last_used_at
                        ? `last used ${formatDateTime(passkey.last_used_at)}`
                        : 'never used'}
                    </p>
                  </div>
                  <button
                    type="button"
                    onClick={() => setRenaming({ id: passkey.id, name: passkey.name })}
                    aria-label={`Rename ${passkey.name}`}
                    title={`Rename ${passkey.name}`}
                    className="p-2 text-muted hover:text-fg transition cursor-pointer"
                  >
                    <Pencil className="w-4 h-4" />
                  </button>
                  <button
                    type="button"
                    onClick={() => {
                      setError(null);
                      setPending({ kind: 'remove', id: passkey.id, name: passkey.name });
                    }}
                    aria-label={`Remove ${passkey.name}`}
                    title={`Remove ${passkey.name}`}
                    data-qa="passkey-remove"
                    className="p-2 text-muted hover:text-danger transition cursor-pointer"
                  >
                    <Trash2 className="w-4 h-4" />
                  </button>
                </>
              )}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

function PanelHeading() {
  return (
    <h3 className="text-sm font-medium text-fg flex items-center gap-2">
      <Fingerprint className="w-4 h-4 text-accent" />
      Passkeys
    </h3>
  );
}
