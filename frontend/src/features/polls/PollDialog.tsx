import { useState, type DragEvent, type FormEvent, type KeyboardEvent } from 'react';
import { GripVertical, Plus, X } from 'lucide-react';
import { Modal } from '@/shared/components/Modal/Modal';
import { Button } from '@/shared/components/Button/Button';
import { IconButton } from '@/shared/components/IconButton/IconButton';
import { TextInput } from '@/shared/components/Form/TextInput';
import { Checkbox } from '@/shared/components/Form/Checkbox';
import { RadioCards } from '@/shared/components/Form/RadioCards';
import { toUserMessage } from '@/lib/errors';
import { MAX_OPTIONS, MIN_OPTIONS, OPTION_MAX, QUESTION_MAX, moveRow } from './pollCommand';
import { useCreatePoll } from './usePolls';

export interface PollTarget {
  channelId: string;
  threadParentId?: string;
  instanceUrl?: string;
}

interface Props {
  target: PollTarget;
  onClose: () => void;
}

interface Row {
  key: string;
  label: string;
}

const blankRow = (): Row => ({ key: crypto.randomUUID(), label: '' });

export function PollDialog({ target, onClose }: Props) {
  const [question, setQuestion] = useState('');
  const [rows, setRows] = useState<Row[]>(() => [blankRow(), blankRow()]);
  const [multiple, setMultiple] = useState(false);
  const [anonymous, setAnonymous] = useState(false);
  const [closesAt, setClosesAt] = useState('');
  const [dragging, setDragging] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);
  const create = useCreatePoll(target.channelId, target.instanceUrl);

  const setLabel = (index: number, label: string) =>
    setRows((current) => current.map((row, i) => (i === index ? { ...row, label } : row)));

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();
    const options = rows.map((row) => row.label.trim()).filter(Boolean);
    if (!question.trim()) {
      setError('Ask a question.');
      return;
    }
    if (options.length < MIN_OPTIONS) {
      setError(`Give at least ${MIN_OPTIONS} options.`);
      return;
    }
    setError(null);
    try {
      await create.mutateAsync({
        question: question.trim(),
        options,
        multiple_choice: multiple,
        anonymous,
        closes_at: closesAt ? new Date(closesAt).toISOString() : null,
        thread_parent_id: target.threadParentId ?? null,
      });
      onClose();
    } catch (err) {
      setError(toUserMessage(err, "Couldn't create the poll"));
    }
  };

  const onOptionKey = (index: number) => (e: KeyboardEvent<HTMLInputElement>) => {
    if (!e.altKey || (e.key !== 'ArrowUp' && e.key !== 'ArrowDown')) return;
    e.preventDefault();
    setRows((current) => moveRow(current, index, index + (e.key === 'ArrowUp' ? -1 : 1)));
  };

  const dropOn = (index: number) => (e: DragEvent<HTMLLIElement>) => {
    e.preventDefault();
    if (dragging !== null) setRows((current) => moveRow(current, dragging, index));
    setDragging(null);
  };

  return (
    <Modal
      title="Create a poll"
      onClose={onClose}
      dataQa="poll-dialog"
      onSubmit={handleSubmit}
      dismissLabel="Cancel"
      actions={
        <Button type="submit" variant="primary" loading={create.isPending} data-qa="poll-dialog-submit">
          Create poll
        </Button>
      }
    >
      <div className="space-y-5">
        <TextInput
          label="Question"
          value={question}
          onChange={(e) => {
            setQuestion(e.target.value);
            if (error) setError(null);
          }}
          placeholder="Lunch Thursday or Friday?"
          maxLength={QUESTION_MAX}
          counter
          error={error}
          autoComplete="off"
          dataQa="poll-dialog-question"
          data-autofocus
        />

        <fieldset>
          <legend className="mb-1.5 text-sm font-medium text-fg-dim">Options</legend>
          <ol className="space-y-2">
            {rows.map((row, index) => (
              <li
                key={row.key}
                draggable
                onDragStart={() => setDragging(index)}
                onDragEnd={() => setDragging(null)}
                onDragOver={(e) => e.preventDefault()}
                onDrop={dropOn(index)}
                data-qa="poll-dialog-option"
                className={`flex items-center gap-1.5 ${dragging === index ? 'opacity-50' : ''}`}
              >
                <GripVertical className="h-4 w-4 shrink-0 cursor-grab text-faint" aria-hidden="true" />
                <TextInput
                  label={`Option ${index + 1}`}
                  labelHidden
                  value={row.label}
                  onChange={(e) => setLabel(index, e.target.value)}
                  onKeyDown={onOptionKey(index)}
                  placeholder={`Option ${index + 1}`}
                  maxLength={OPTION_MAX}
                  autoComplete="off"
                  className="flex-1"
                  dataQa={`poll-dialog-option-${index}`}
                />
                <IconButton
                  label={`Remove option ${index + 1}`}
                  icon={<X className="h-4 w-4" />}
                  tone="danger"
                  disabled={rows.length <= MIN_OPTIONS}
                  onClick={() => setRows((current) => current.filter((_, i) => i !== index))}
                />
              </li>
            ))}
          </ol>
          <p className="mt-1.5 text-xs text-muted">Drag to reorder, or Alt + ↑ / ↓ in an option.</p>
          {rows.length < MAX_OPTIONS && (
            <Button
              size="sm"
              variant="ghost"
              icon={<Plus className="h-3.5 w-3.5" />}
              onClick={() => setRows((current) => [...current, blankRow()])}
              className="mt-1"
              data-qa="poll-dialog-add-option"
            >
              Add option
            </Button>
          )}
        </fieldset>

        <RadioCards
          label="Answers"
          value={multiple ? 'multiple' : 'single'}
          onChange={(value) => setMultiple(value === 'multiple')}
          options={[
            { value: 'single', label: 'One answer', description: 'Each person picks one option.' },
            {
              value: 'multiple',
              label: 'Any number',
              description: 'Each person picks as many as they like.',
              dataQa: 'poll-dialog-multiple',
            },
          ]}
        />

        <Checkbox
          label="Anonymous"
          description="Everyone sees the counts; nobody, you included, sees who voted for what."
          checked={anonymous}
          onChange={setAnonymous}
          dataQa="poll-dialog-anonymous"
        />

        <TextInput
          label="Closes"
          optional
          type="datetime-local"
          value={closesAt}
          onChange={(e) => setClosesAt(e.target.value)}
          hint="Leave empty to keep it open until you close it."
          dataQa="poll-dialog-closes"
        />
      </div>
    </Modal>
  );
}
