import { useState, type FormEvent, type KeyboardEvent } from 'react';
import { useTranslation } from 'react-i18next';
import { Plus, X } from 'lucide-react';
import { Modal } from '@/shared/components/Modal/Modal';
import { Button } from '@/shared/components/Button/Button';
import { IconButton } from '@/shared/components/IconButton/IconButton';
import { TextInput } from '@/shared/components/Form/TextInput';
import { Select } from '@/shared/components/Form/Select';
import { Checkbox } from '@/shared/components/Form/Checkbox';
import {
  moveItem,
  useCreateProfileField,
  useUpdateProfileField,
  type ProfileField,
  type ProfileFieldKind,
} from '@/features/people';
import { toUserMessage } from '@/lib/errors';
import { fieldDraft, fieldRequest, readyToSave, type OptionDraft } from './profileFieldDraft';

const KINDS: ProfileFieldKind[] = ['text', 'link', 'date', 'options', 'person'];

interface Props {
  workspaceId: string;
  instanceUrl?: string;
  field: ProfileField | null;
  cardFull: boolean;
  onClose: () => void;
}

export function ProfileFieldDialog({ workspaceId, instanceUrl, field, cardFull, onClose }: Props) {
  const { t } = useTranslation(['workspace', 'common']);
  const create = useCreateProfileField(workspaceId, instanceUrl);
  const update = useUpdateProfileField(workspaceId, instanceUrl);
  const [draft, setDraft] = useState(() => fieldDraft(field));
  const [error, setError] = useState<string | null>(null);
  const saving = create.isPending || update.isPending;

  const setOptions = (change: (options: OptionDraft[]) => OptionDraft[]) =>
    setDraft((d) => ({ ...d, options: change(d.options) }));

  const onOptionKey = (index: number) => (e: KeyboardEvent<HTMLInputElement>) => {
    if (!e.altKey || (e.key !== 'ArrowUp' && e.key !== 'ArrowDown')) return;
    e.preventDefault();
    setOptions((options) => moveItem(options, index, index + (e.key === 'ArrowUp' ? -1 : 1)));
  };

  const submit = (e: FormEvent) => {
    e.preventDefault();
    if (!readyToSave(draft)) return;
    setError(null);
    const failed = (err: unknown) => setError(toUserMessage(err, t('profileFields.saveFailed')));
    if (field) {
      update.mutate(
        { fieldId: field.id, body: fieldRequest(draft) },
        { onSuccess: onClose, onError: failed },
      );
    } else {
      create.mutate({ ...fieldRequest(draft), kind: draft.kind }, { onSuccess: onClose, onError: failed });
    }
  };

  return (
    <Modal
      title={field ? t('profileFields.editTitle') : t('profileFields.addTitle')}
      onClose={onClose}
      onSubmit={submit}
      dataQa="profile-field-dialog"
      dismissLabel={t('cancel', { ns: 'common' })}
      actions={
        <Button
          type="submit"
          variant="primary"
          loading={saving}
          disabled={!readyToSave(draft)}
          data-qa="profile-field-save"
        >
          {field ? t('profileFields.save') : t('profileFields.add')}
        </Button>
      }
    >
      <div className="space-y-5">
        <TextInput
          label={t('profileFields.label')}
          value={draft.label}
          onChange={(e) => setDraft((d) => ({ ...d, label: e.target.value }))}
          placeholder={t('profileFields.labelPlaceholder')}
          maxLength={50}
          counter
          autoComplete="off"
          dataQa="profile-field-label"
          data-autofocus
        />
        <TextInput
          label={t('profileFields.hint')}
          optional
          value={draft.hint}
          onChange={(e) => setDraft((d) => ({ ...d, hint: e.target.value }))}
          placeholder={t('profileFields.hintPlaceholder')}
          maxLength={150}
          autoComplete="off"
          dataQa="profile-field-hint"
        />
        <Select
          label={t('profileFields.kind')}
          value={draft.kind}
          disabled={!!field}
          hint={field ? t('profileFields.kindFixed') : undefined}
          onChange={(e) => setDraft((d) => ({ ...d, kind: e.target.value as ProfileFieldKind }))}
          dataQa="profile-field-kind"
        >
          {KINDS.map((kind) => (
            <option key={kind} value={kind}>
              {t(`profileFields.kinds.${kind}`)}
            </option>
          ))}
        </Select>

        {draft.kind === 'options' && (
          <fieldset>
            <legend className="mb-1.5 text-sm font-medium text-fg-dim">{t('profileFields.options')}</legend>
            <ol className="space-y-2">
              {draft.options.map((option, index) => (
                <li key={option.key} className="flex items-center gap-1.5" data-qa="profile-field-option">
                  <TextInput
                    label={t('profileFields.option', { n: index + 1 })}
                    labelHidden
                    value={option.label}
                    onChange={(e) =>
                      setOptions((options) =>
                        options.map((o, i) => (i === index ? { ...o, label: e.target.value } : o)),
                      )
                    }
                    onKeyDown={onOptionKey(index)}
                    placeholder={t('profileFields.option', { n: index + 1 })}
                    maxLength={80}
                    autoComplete="off"
                    className="flex-1"
                    dataQa="profile-field-option-label"
                  />
                  <IconButton
                    label={t('profileFields.removeOption', { n: index + 1 })}
                    icon={<X className="h-4 w-4" />}
                    onClick={() => setOptions((options) => options.filter((_, i) => i !== index))}
                    disabled={draft.options.length === 1}
                  />
                </li>
              ))}
            </ol>
            <Button
              variant="ghost"
              size="sm"
              className="mt-2"
              icon={<Plus className="h-4 w-4" />}
              onClick={() => setOptions((options) => [...options, { key: crypto.randomUUID(), label: '' }])}
              disabled={draft.options.length >= 100}
              data-qa="profile-field-add-option"
            >
              {t('profileFields.addOption')}
            </Button>
            {field && <p className="mt-2 text-xs text-muted">{t('profileFields.optionsRemoved')}</p>}
          </fieldset>
        )}

        <Checkbox
          label={t('profileFields.adminsOnly')}
          description={t('profileFields.adminsOnlyHint')}
          checked={draft.adminsOnly}
          onChange={(adminsOnly) => setDraft((d) => ({ ...d, adminsOnly }))}
          dataQa="profile-field-admins-only"
        />
        <Checkbox
          label={t('profileFields.onCard')}
          description={
            cardFull && !draft.onCard ? t('profileFields.cardFull') : t('profileFields.onCardHint')
          }
          checked={draft.onCard}
          disabled={cardFull && !draft.onCard}
          onChange={(onCard) => setDraft((d) => ({ ...d, onCard }))}
          dataQa="profile-field-on-card"
        />
        {error && (
          <p role="alert" className="text-sm text-danger" data-qa="profile-field-error">
            {error}
          </p>
        )}
      </div>
    </Modal>
  );
}
