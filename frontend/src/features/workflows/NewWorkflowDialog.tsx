import { useState, type FormEvent } from 'react';
import { Modal } from '@/shared/components/Modal/Modal';
import { Button } from '@/shared/components/Button/Button';
import { RadioCards } from '@/shared/components/Form/RadioCards';
import { WORKFLOW_TEMPLATES, type Draft, type TemplateId } from './templates';

interface Props {
  channelId: string;
  onPick: (draft: Draft) => void;
  onClose: () => void;
}

export function NewWorkflowDialog({ channelId, onPick, onClose }: Props) {
  const [template, setTemplate] = useState<TemplateId>('blank');

  const handleSubmit = (e: FormEvent) => {
    e.preventDefault();
    const chosen = WORKFLOW_TEMPLATES.find((t) => t.id === template) ?? WORKFLOW_TEMPLATES[0];
    onPick(chosen.draft(channelId));
  };

  return (
    <Modal
      title="New workflow"
      description="Start from scratch or from a template you can change before publishing."
      onClose={onClose}
      onSubmit={handleSubmit}
      dataQa="new-workflow-dialog"
      dismissLabel="Cancel"
      actions={
        <Button type="submit" variant="primary" data-qa="new-workflow-submit">
          Continue
        </Button>
      }
    >
      <RadioCards
        label="Start from"
        value={template}
        onChange={setTemplate}
        options={WORKFLOW_TEMPLATES.map((t) => ({
          value: t.id,
          label: t.label,
          description: t.description,
          dataQa: `new-workflow-template-${t.id}`,
        }))}
      />
    </Modal>
  );
}
