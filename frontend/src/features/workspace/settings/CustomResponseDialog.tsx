import { useState, type FormEvent } from 'react';
import { useTranslation } from 'react-i18next';
import { Plus, X } from 'lucide-react';
import { Modal } from '@/shared/components/Modal/Modal';
import { Button } from '@/shared/components/Button/Button';
import { IconButton } from '@/shared/components/IconButton/IconButton';
import { TextInput } from '@/shared/components/Form/TextInput';
import { TextArea } from '@/shared/components/Form/TextArea';
import {
  MAX_ANSWER,
  MAX_ANSWERS,
  MAX_TRIGGER,
  MAX_TRIGGERS,
  useSaveCustomResponse,
  type CustomResponse,
} from '@/features/custom-responses';
import { toUserMessage } from '@/lib/errors';
import { emptyLine, readyToSave, responseDraft, responseRequest, type LineDraft } from './responseDraft';

interface Props {
  workspaceId: string;
  instanceUrl?: string;
  botName: string;
  response: CustomResponse | null;
  onClose: () => void;
}

type Part = 'triggers' | 'answers';

export function CustomResponseDialog({ workspaceId, instanceUrl, botName, response, onClose }: Props) {
  const { t } = useTranslation(['workspace', 'common']);
  const save = useSaveCustomResponse(workspaceId, instanceUrl);
  const [draft, setDraft] = useState(() => responseDraft(response));
  const [error, setError] = useState<string | null>(null);

  const change = (part: Part, edit: (lines: LineDraft[]) => LineDraft[]) =>
    setDraft((d) => ({ ...d, [part]: edit(d[part]) }));
  const setText = (part: Part, index: number, text: string) =>
    change(part, (lines) => lines.map((l, i) => (i === index ? { ...l, text } : l)));

  const submit = (e: FormEvent) => {
    e.preventDefault();
    if (!readyToSave(draft)) return;
    setError(null);
    save.mutate(
      { id: response?.id ?? null, body: responseRequest(draft) },
      {
        onSuccess: onClose,
        onError: (err) => setError(toUserMessage(err, t('customResponses.saveFailed'))),
      },
    );
  };

  return (
    <Modal
      title={response ? t('customResponses.editTitle') : t('customResponses.addTitle')}
      onClose={onClose}
      onSubmit={submit}
      dataQa="custom-response-dialog"
      dismissLabel={t('cancel', { ns: 'common' })}
      actions={
        <Button
          type="submit"
          variant="primary"
          loading={save.isPending}
          disabled={!readyToSave(draft)}
          data-qa="custom-response-save"
        >
          {response ? t('customResponses.save') : t('customResponses.add')}
        </Button>
      }
    >
      <div className="space-y-5">
        <fieldset>
          <legend className="text-sm font-medium text-fg-dim">{t('customResponses.triggers')}</legend>
          <p className="mb-2 text-xs text-muted">{t('customResponses.triggersHint')}</p>
          <ol className="space-y-2">
            {draft.triggers.map((trigger, index) => (
              <li key={trigger.key} className="flex items-center gap-1.5" data-qa="custom-response-trigger">
                <TextInput
                  label={t('customResponses.trigger', { n: index + 1 })}
                  labelHidden
                  value={trigger.text}
                  onChange={(e) => setText('triggers', index, e.target.value)}
                  placeholder={t('customResponses.triggerPlaceholder')}
                  maxLength={MAX_TRIGGER}
                  autoComplete="off"
                  className="flex-1"
                  dataQa="custom-response-trigger-text"
                  data-autofocus={index === 0 ? true : undefined}
                />
                <IconButton
                  label={t('customResponses.removeTrigger', { n: index + 1 })}
                  icon={<X className="h-4 w-4" />}
                  onClick={() => change('triggers', (lines) => lines.filter((_, i) => i !== index))}
                  disabled={draft.triggers.length === 1}
                />
              </li>
            ))}
          </ol>
          <Button
            variant="ghost"
            size="sm"
            className="mt-2"
            icon={<Plus className="h-4 w-4" />}
            onClick={() => change('triggers', (lines) => [...lines, emptyLine()])}
            disabled={draft.triggers.length >= MAX_TRIGGERS}
            data-qa="custom-response-add-trigger"
          >
            {t('customResponses.addTrigger')}
          </Button>
        </fieldset>

        <fieldset>
          <legend className="text-sm font-medium text-fg-dim">
            {t('customResponses.answers', { name: botName })}
          </legend>
          <p className="mb-2 text-xs text-muted">{t('customResponses.answersHint')}</p>
          <ol className="space-y-2">
            {draft.answers.map((answer, index) => (
              <li key={answer.key} className="flex items-start gap-1.5" data-qa="custom-response-answer">
                <TextArea
                  label={t('customResponses.answer', { n: index + 1 })}
                  labelHidden
                  value={answer.text}
                  onChange={(e) => setText('answers', index, e.target.value)}
                  placeholder={t('customResponses.answerPlaceholder')}
                  maxLength={MAX_ANSWER}
                  rows={2}
                  className="flex-1"
                  dataQa="custom-response-answer-text"
                />
                <IconButton
                  label={t('customResponses.removeAnswer', { n: index + 1 })}
                  icon={<X className="h-4 w-4" />}
                  onClick={() => change('answers', (lines) => lines.filter((_, i) => i !== index))}
                  disabled={draft.answers.length === 1}
                />
              </li>
            ))}
          </ol>
          <Button
            variant="ghost"
            size="sm"
            className="mt-2"
            icon={<Plus className="h-4 w-4" />}
            onClick={() => change('answers', (lines) => [...lines, emptyLine()])}
            disabled={draft.answers.length >= MAX_ANSWERS}
            data-qa="custom-response-add-answer"
          >
            {t('customResponses.addAnswer')}
          </Button>
        </fieldset>

        {error && (
          <p role="alert" className="text-sm text-danger" data-qa="custom-response-error">
            {error}
          </p>
        )}
      </div>
    </Modal>
  );
}
