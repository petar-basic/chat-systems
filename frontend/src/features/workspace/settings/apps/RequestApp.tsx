import { useState, type FormEvent } from 'react';
import { Button } from '@/shared/components/Button/Button';
import { TextInput } from '@/shared/components/Form/TextInput';
import { TextArea } from '@/shared/components/Form/TextArea';
import { toUserMessage } from '@/lib/errors';
import { useRequestApp } from '@/features/workspace/hooks/useAppDirectory';

export function RequestApp({ workspaceId }: { workspaceId: string }) {
  const request = useRequestApp(workspaceId);
  const [name, setName] = useState('');
  const [reason, setReason] = useState('');
  const [sent, setSent] = useState<string | null>(null);

  const submit = (e: FormEvent) => {
    e.preventDefault();
    if (!name.trim()) return;
    request.mutate(
      { name: name.trim(), reason: reason.trim() },
      {
        onSuccess: () => {
          setSent(name.trim());
          setName('');
          setReason('');
        },
      },
    );
  };

  return (
    <section className="space-y-3" data-qa="app-request">
      <div>
        <h3 className="text-sm font-semibold text-fg-soft">Ask for an app</h3>
        <p className="text-xs text-muted">
          Only an admin can add one. Your request lands in their notifications.
        </p>
      </div>
      <form onSubmit={submit} noValidate className="space-y-3 rounded-lg border border-line/50 p-3">
        <TextInput
          label="What should be added?"
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="PagerDuty"
          dataQa="app-request-name"
        />
        <TextArea
          label="Why"
          optional
          rows={2}
          value={reason}
          onChange={(e) => setReason(e.target.value)}
          dataQa="app-request-reason"
        />
        <Button
          type="submit"
          variant="primary"
          loading={request.isPending}
          disabled={!name.trim()}
          data-qa="app-request-send"
        >
          Send request
        </Button>
      </form>
      {sent && (
        <p className="text-sm text-success" data-qa="app-request-sent">
          Asked the admins for {sent}.
        </p>
      )}
      {request.error && (
        <p role="alert" className="text-sm text-danger">
          {toUserMessage(request.error, 'The request was not sent')}
        </p>
      )}
    </section>
  );
}
