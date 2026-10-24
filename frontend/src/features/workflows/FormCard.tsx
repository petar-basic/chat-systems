import { useState } from 'react';
import { ClipboardList } from 'lucide-react';
import { Button } from '@/shared/components/Button/Button';
import { formatDateTime } from '@/lib/datetime';
import { displayNameOf } from '@/lib/userHelpers';
import { useUserCache } from '@/stores/users';
import { FormDialog } from './FormDialog';
import type { FormRequest } from './workflowModel';

interface Props {
  form: FormRequest;
  currentUserId: string;
}

export function FormCard({ form, currentUserId }: Props) {
  const { getUser } = useUserCache();
  const [filling, setFilling] = useState(false);
  const [sent, setSent] = useState(false);
  const mine = form.recipient_id === currentUserId;
  const questions = `${form.fields.length} ${form.fields.length === 1 ? 'question' : 'questions'}`;

  const status = () => {
    if (form.submitted_at) return `Filled in ${formatDateTime(form.submitted_at)}`;
    if (sent) return 'Filled in';
    if (form.expired) return 'This form expired';
    if (mine) return `${questions} · open until ${formatDateTime(form.expires_at)}`;
    return `Waiting for ${displayNameOf(getUser(form.recipient_id)?.display_name)}`;
  };
  const open = !form.submitted_at && !sent && !form.expired;

  return (
    <div
      data-qa="workflow-form-card"
      className="mt-1 max-w-md rounded-xl border border-line bg-surface p-3"
      aria-label={`Form: ${form.title}`}
      role="group"
    >
      <div className="flex items-start gap-2">
        <ClipboardList className="mt-0.5 h-4 w-4 shrink-0 text-accent-soft" aria-hidden="true" />
        <div className="min-w-0 flex-1">
          <p className="break-words text-sm font-semibold text-fg" data-qa="workflow-form-card-title">
            {form.title}
          </p>
          <p className="text-xs text-muted" data-qa="workflow-form-card-status">
            {status()}
          </p>
        </div>
        {open && mine && (
          <Button size="sm" variant="primary" onClick={() => setFilling(true)} data-qa="workflow-form-fill">
            Fill in
          </Button>
        )}
      </div>
      {filling && (
        <FormDialog form={form} onSubmitted={() => setSent(true)} onClose={() => setFilling(false)} />
      )}
    </div>
  );
}
