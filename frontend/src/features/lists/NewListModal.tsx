import { useState, type FormEvent } from 'react';
import { Modal } from '@/shared/components/Modal/Modal';
import { Button } from '@/shared/components/Button/Button';
import { TextInput } from '@/shared/components/Form/TextInput';
import { RadioCards } from '@/shared/components/Form/RadioCards';
import { toUserMessage } from '@/lib/errors';
import { LIST_TEMPLATES, type TemplateId } from './templates';
import { useCreateList } from './useLists';
import type { ListDetail } from './listModel';

interface Props {
  workspaceId: string;
  channelId: string;
  instanceUrl?: string;
  onCreated: (detail: ListDetail) => void;
  onClose: () => void;
}

const TITLE_MAX = 200;

export function NewListModal({ workspaceId, channelId, instanceUrl, onCreated, onClose }: Props) {
  const [template, setTemplate] = useState<TemplateId>('blank');
  const [title, setTitle] = useState('');
  const [error, setError] = useState<string | null>(null);
  const create = useCreateList(workspaceId, instanceUrl);
  const chosen = LIST_TEMPLATES.find((t) => t.id === template) ?? LIST_TEMPLATES[0];

  const handleSubmit = (e: FormEvent) => {
    e.preventDefault();
    setError(null);
    create.mutate(
      { title: title.trim() || chosen.title, channel_id: channelId, columns: chosen.columns },
      {
        onSuccess: (detail) => {
          onCreated(detail);
          onClose();
        },
        onError: (err) => setError(toUserMessage(err, "Couldn't create the list")),
      },
    );
  };

  return (
    <Modal
      title="New list"
      onClose={onClose}
      onSubmit={handleSubmit}
      dataQa="new-list-dialog"
      dismissLabel="Cancel"
      actions={
        <Button type="submit" variant="primary" loading={create.isPending} data-qa="new-list-submit">
          Create list
        </Button>
      }
    >
      <div className="space-y-5">
        <TextInput
          label="Title"
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          placeholder={chosen.title}
          maxLength={TITLE_MAX}
          error={error}
          autoComplete="off"
          dataQa="new-list-title"
          data-autofocus
        />
        <RadioCards
          label="Start from"
          value={template}
          onChange={setTemplate}
          options={LIST_TEMPLATES.map((t) => ({
            value: t.id,
            label: t.label,
            description: t.description,
            dataQa: `new-list-template-${t.id}`,
          }))}
        />
      </div>
    </Modal>
  );
}
