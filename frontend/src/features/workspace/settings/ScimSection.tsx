import { useState, type FormEvent } from 'react';
import { KeyRound, RefreshCw, Trash2 } from 'lucide-react';
import { Spinner } from '@/shared/components/Spinner/Spinner';
import { Button } from '@/shared/components/Button/Button';
import { TextInput } from '@/shared/components/Form/TextInput';
import { CopyField } from '@/shared/components/CopyField/CopyField';
import { ConfirmDialog } from '@/shared/components/ConfirmDialog/ConfirmDialog';
import { toUserMessage } from '@/lib/errors';
import { formatDateTime } from '@/lib/datetime';
import {
  useMintScimToken,
  useRevokeScimToken,
  useRotateScimToken,
  useScimTokens,
  type MintedScimToken,
  type ScimToken,
} from '@/hooks/queries/useScimTokens';

interface Props {
  instanceUrl?: string;
}

function TokenRow({
  token,
  busy,
  onRotate,
  onRevoke,
}: {
  token: ScimToken;
  busy: boolean;
  onRotate: () => void;
  onRevoke: () => Promise<unknown>;
}) {
  const [confirmRevoke, setConfirmRevoke] = useState(false);
  const revoked = !!token.revoked_at;
  const name = token.description || 'Unnamed token';

  return (
    <div className="flex items-center gap-3 px-4 py-3 border-b border-line/40" data-qa="scim-token-row">
      <KeyRound className={`w-4 h-4 shrink-0 ${revoked ? 'text-muted' : 'text-accent-soft'}`} />
      <div className="min-w-0 flex-1">
        <div className={`text-sm truncate ${revoked ? 'text-muted line-through' : 'text-fg-soft'}`}>
          {name}
        </div>
        <div className="text-xs text-muted" data-qa="scim-token-state">
          {revoked
            ? `Revoked ${formatDateTime(token.revoked_at ?? '')}`
            : token.last_used_at
              ? `Last used ${formatDateTime(token.last_used_at)}`
              : `Created ${formatDateTime(token.created_at)}, never used`}
        </div>
      </div>
      {!revoked && (
        <div className="flex items-center gap-1 shrink-0">
          <button
            onClick={onRotate}
            disabled={busy}
            aria-label={`Rotate ${name}`}
            title="Rotate"
            data-qa="scim-token-rotate"
            className="p-1.5 rounded text-muted hover:text-fg hover:bg-raised transition cursor-pointer disabled:opacity-50"
          >
            <RefreshCw className="w-4 h-4" />
          </button>
          <button
            onClick={() => setConfirmRevoke(true)}
            disabled={busy}
            aria-label={`Revoke ${name}`}
            title="Revoke"
            data-qa="scim-token-revoke"
            className="p-1.5 rounded text-muted hover:text-danger hover:bg-raised transition cursor-pointer disabled:opacity-50"
          >
            <Trash2 className="w-4 h-4" />
          </button>
        </div>
      )}
      {confirmRevoke && (
        <ConfirmDialog
          title={`Revoke ${name}?`}
          description="The identity provider using it stops provisioning at once."
          confirmLabel="Revoke token"
          onConfirm={async () => {
            await onRevoke();
          }}
          onClose={() => setConfirmRevoke(false)}
          dataQa="scim-revoke-dialog"
        />
      )}
    </div>
  );
}

export function ScimSection({ instanceUrl }: Props) {
  const { data: tokens = [], isLoading, error } = useScimTokens(instanceUrl);
  const mint = useMintScimToken(instanceUrl);
  const rotate = useRotateScimToken(instanceUrl);
  const revoke = useRevokeScimToken(instanceUrl);
  const [description, setDescription] = useState('');
  const [minted, setMinted] = useState<MintedScimToken | null>(null);

  const submit = (e: FormEvent) => {
    e.preventDefault();
    mint.mutate(description.trim(), {
      onSuccess: (token) => {
        setMinted(token);
        setDescription('');
      },
    });
  };

  const failure = mint.error ?? rotate.error ?? revoke.error;

  return (
    <div className="-mx-4 flex flex-col" data-qa="scim-section">
      <div className="px-4 pb-4 border-b border-line/50 space-y-3">
        <p className="text-sm text-muted">
          An identity provider such as Okta or Entra ID creates, renames and deactivates accounts on this
          instance through <code className="text-fg-soft">/api/scim/v2</code> with one of these tokens.
        </p>
        <form onSubmit={submit} className="flex items-end gap-2">
          <TextInput
            label="Token name"
            optional
            value={description}
            onChange={(e) => setDescription(e.target.value)}
            placeholder="Okta"
            dataQa="scim-token-description"
            className="flex-1"
          />
          <Button type="submit" variant="primary" loading={mint.isPending} data-qa="scim-token-mint">
            Create token
          </Button>
        </form>
        {minted && (
          <div className="rounded-lg border border-warning/40 bg-warning/5 p-3" data-qa="scim-token-minted">
            <p className="text-sm text-fg-soft">
              Copy this now. It is not shown again, and nobody can read it back later.
            </p>
            <CopyField
              label={minted.description || 'SCIM token'}
              value={minted.token}
              valueQa="scim-token-value"
              copyQa="scim-token-copy"
            />
            <button
              onClick={() => setMinted(null)}
              data-qa="scim-token-dismiss"
              className="mt-2 text-xs text-muted hover:text-fg transition cursor-pointer"
            >
              I have stored it
            </button>
          </div>
        )}
        {failure && (
          <p role="alert" className="text-sm text-danger">
            {toUserMessage(failure, 'The token change failed')}
          </p>
        )}
      </div>

      {isLoading ? (
        <div className="flex justify-center py-8">
          <Spinner className="text-accent" />
        </div>
      ) : error ? (
        <p className="px-4 py-6 text-sm text-danger">{toUserMessage(error, 'Failed to load tokens')}</p>
      ) : tokens.length === 0 ? (
        <p className="px-4 py-6 text-sm text-muted text-center">No tokens yet.</p>
      ) : (
        tokens.map((token) => (
          <TokenRow
            key={token.id}
            token={token}
            busy={rotate.isPending || revoke.isPending}
            onRotate={() => rotate.mutate(token.id, { onSuccess: setMinted })}
            onRevoke={() => revoke.mutateAsync(token.id)}
          />
        ))
      )}
    </div>
  );
}
