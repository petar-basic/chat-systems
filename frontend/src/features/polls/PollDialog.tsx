import { useState, type DragEvent, type FormEvent, type KeyboardEvent } from 'react';
import { useTranslation } from 'react-i18next';
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
  const { t } = useTranslation(['polls', 'common']);
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

  const handleSubmit = (e: FormEvent) => {
    e.preventDefault();
    const options = rows.map((row) => row.label.trim()).filter(Boolean);
    if (!question.trim()) {
      setError(t('dialog.needQuestion'));
      return;
    }
    if (options.length < MIN_OPTIONS) {
      setError(t('dialog.needOptions', { min: MIN_OPTIONS }));
      return;
    }
    setError(null);
    create.mutate(
      {
        question: question.trim(),
        options,
        multiple_choice: multiple,
        anonymous,
        closes_at: closesAt ? new Date(closesAt).toISOString() : null,
        thread_parent_id: target.threadParentId ?? null,
      },
      {
        onSuccess: onClose,
        onError: (err) => setError(toUserMessage(err, t('dialog.failed'))),
      },
    );
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
      title={t('dialog.title')}
      onClose={onClose}
      dataQa="poll-dialog"
      onSubmit={handleSubmit}
      dismissLabel={t('cancel', { ns: 'common' })}
      actions={
        <Button type="submit" variant="primary" loading={create.isPending} data-qa="poll-dialog-submit">
          {t('dialog.submit')}
        </Button>
      }
    >
      <div className="space-y-5">
        <TextInput
          label={t('dialog.question')}
          value={question}
          onChange={(e) => {
            setQuestion(e.target.value);
            if (error) setError(null);
          }}
          placeholder={t('dialog.questionPlaceholder')}
          maxLength={QUESTION_MAX}
          counter
          error={error}
          autoComplete="off"
          dataQa="poll-dialog-question"
          data-autofocus
        />

        <fieldset>
          <legend className="mb-1.5 text-sm font-medium text-fg-dim">{t('dialog.options')}</legend>
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
                  label={t('dialog.option', { n: index + 1 })}
                  labelHidden
                  value={row.label}
                  onChange={(e) => setLabel(index, e.target.value)}
                  onKeyDown={onOptionKey(index)}
                  placeholder={t('dialog.option', { n: index + 1 })}
                  maxLength={OPTION_MAX}
                  autoComplete="off"
                  className="flex-1"
                  dataQa={`poll-dialog-option-${index}`}
                />
                <IconButton
                  label={t('dialog.removeOption', { n: index + 1 })}
                  icon={<X className="h-4 w-4" />}
                  tone="danger"
                  disabled={rows.length <= MIN_OPTIONS}
                  onClick={() => setRows((current) => current.filter((_, i) => i !== index))}
                />
              </li>
            ))}
          </ol>
          <p className="mt-1.5 text-xs text-muted">{t('dialog.reorderHint')}</p>
          {rows.length < MAX_OPTIONS && (
            <Button
              size="sm"
              variant="ghost"
              icon={<Plus className="h-3.5 w-3.5" />}
              onClick={() => setRows((current) => [...current, blankRow()])}
              className="mt-1"
              data-qa="poll-dialog-add-option"
            >
              {t('dialog.addOption')}
            </Button>
          )}
        </fieldset>

        <RadioCards
          label={t('dialog.answers')}
          value={multiple ? 'multiple' : 'single'}
          onChange={(value) => setMultiple(value === 'multiple')}
          options={[
            { value: 'single', label: t('dialog.single'), description: t('dialog.singleHint') },
            {
              value: 'multiple',
              label: t('dialog.multiple'),
              description: t('dialog.multipleHint'),
              dataQa: 'poll-dialog-multiple',
            },
          ]}
        />

        <Checkbox
          label={t('dialog.anonymous')}
          description={t('dialog.anonymousHint')}
          checked={anonymous}
          onChange={setAnonymous}
          dataQa="poll-dialog-anonymous"
        />

        <TextInput
          label={t('dialog.closes')}
          optional
          type="datetime-local"
          value={closesAt}
          onChange={(e) => setClosesAt(e.target.value)}
          hint={t('dialog.closesHint')}
          dataQa="poll-dialog-closes"
        />
      </div>
    </Modal>
  );
}
