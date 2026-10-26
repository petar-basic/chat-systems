import { useState, type FormEvent } from 'react';
import { Modal } from '@/shared/components/Modal/Modal';
import { Button } from '@/shared/components/Button/Button';
import { Select } from '@/shared/components/Form/Select';
import { TextArea } from '@/shared/components/Form/TextArea';
import { TextInput } from '@/shared/components/Form/TextInput';
import { toUserMessage } from '@/lib/errors';
import { displayNameOf } from '@/lib/userHelpers';
import { useWorkspaceStore } from '@/stores/workspace';
import { useWorkspaceMembers } from '@/features/workspace';
import { useSubmitForm } from './useWorkflows';
import { LONG_ANSWER_MAX, SHORT_ANSWER_MAX, type FormField, type FormRequest } from './workflowModel';

interface Props {
  form: FormRequest;
  onSubmitted: () => void;
  onClose: () => void;
}

export function FormDialog({ form, onSubmitted, onClose }: Props) {
  const workspace = useWorkspaceStore((s) => s.currentWorkspace);
  const asksForPerson = form.fields.some((field) => field.kind === 'person');
  const members = useWorkspaceMembers(asksForPerson ? (workspace?.id ?? null) : null, workspace?.instanceUrl);
  const submit = useSubmitForm(workspace?.instanceUrl);
  const [answers, setAnswers] = useState<Record<string, string>>({});
  const [error, setError] = useState<string | null>(null);

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();
    const missing = form.fields.find((field) => field.required && !answers[field.name]?.trim());
    if (missing) {
      setError(`“${missing.label}” needs an answer`);
      return;
    }
    setError(null);
    try {
      await submit.mutateAsync({ runId: form.run_id, position: form.position, answers });
      onSubmitted();
      onClose();
    } catch (err) {
      setError(toUserMessage(err, "Couldn't send the form"));
    }
  };

  const control = (field: FormField, index: number) => {
    const common = {
      label: field.label,
      optional: !field.required,
      value: answers[field.name] ?? '',
      dataQa: 'workflow-form-answer',
    };
    const set = (value: string) => setAnswers((current) => ({ ...current, [field.name]: value }));
    const autofocus = index === 0 ? { 'data-autofocus': true } : {};
    switch (field.kind) {
      case 'short_text':
        return (
          <TextInput
            {...common}
            {...autofocus}
            maxLength={SHORT_ANSWER_MAX}
            onChange={(e) => set(e.target.value)}
          />
        );
      case 'long_text':
        return (
          <TextArea
            {...common}
            {...autofocus}
            rows={4}
            maxLength={LONG_ANSWER_MAX}
            onChange={(e) => set(e.target.value)}
          />
        );
      case 'date':
        return <TextInput {...common} {...autofocus} type="date" onChange={(e) => set(e.target.value)} />;
      case 'select':
        return (
          <Select {...common} onChange={(e) => set(e.target.value)}>
            <option value="">Choose…</option>
            {(field.options ?? []).map((option) => (
              <option key={option} value={option}>
                {option}
              </option>
            ))}
          </Select>
        );
      case 'person':
        return (
          <Select {...common} onChange={(e) => set(e.target.value)}>
            <option value="">Pick a person…</option>
            {(members.data ?? []).map((member) => (
              <option key={member.user_id} value={member.user_id}>
                {displayNameOf(member.display_name)}
              </option>
            ))}
          </Select>
        );
    }
  };

  return (
    <Modal
      title={form.title}
      description="Your answers go back to the workflow that asked."
      onClose={onClose}
      onSubmit={handleSubmit}
      dataQa="workflow-form-dialog"
      dismissLabel="Cancel"
      actions={
        <Button type="submit" variant="primary" loading={submit.isPending} data-qa="workflow-form-submit">
          Submit
        </Button>
      }
    >
      <div className="space-y-4 py-3">
        {form.fields.map((field, index) => (
          <div key={field.name}>{control(field, index)}</div>
        ))}
        {error && (
          <p
            role="alert"
            className="rounded-lg bg-red-500/10 px-3 py-2 text-sm text-danger"
            data-qa="workflow-form-error"
          >
            {error}
          </p>
        )}
      </div>
    </Modal>
  );
}
