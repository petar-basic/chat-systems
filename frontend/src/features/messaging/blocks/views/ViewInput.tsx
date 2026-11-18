import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { TextInput } from '@/shared/components/Form/TextInput';
import { TextArea } from '@/shared/components/Form/TextArea';
import { Select } from '@/shared/components/Form/Select';
import { Checkbox } from '@/shared/components/Form/Checkbox';
import { RadioCards } from '@/shared/components/Form/RadioCards';
import { Combobox } from '@/shared/components/Form/Combobox';
import { useWorkspaceStore } from '@/stores/workspace';
import { useChannelChoices, usePersonChoices } from '../choices';
import type { InputBlock } from './viewValues';

interface Props {
  block: InputBlock;
  value: string[];
  error: string | null;
  onChange: (value: string[]) => void;
}

const one = (value: string) => (value ? [value] : []);

export function ViewInput({ block, value, error, onChange }: Props) {
  const { t } = useTranslation('messages');
  const { t: tc } = useTranslation('common');
  const workspace = useWorkspaceStore((s) => s.currentWorkspace);
  const people = usePersonChoices(workspace?.id ?? null, workspace?.instanceUrl);
  const channels = useChannelChoices(workspace?.id ?? null, workspace?.instanceUrl);
  const element = block.element;
  const field = {
    label: block.label.text,
    optional: block.optional,
    hint: block.hint?.text,
    error,
  };
  const qa = `view-input-${block.block_id}`;

  switch (element.type) {
    case 'plain_text_input':
      return element.multiline ? (
        <TextArea
          {...field}
          value={value[0] ?? ''}
          placeholder={element.placeholder?.text}
          maxLength={element.max_length ?? undefined}
          onChange={(e) => onChange(one(e.target.value))}
          dataQa={qa}
        />
      ) : (
        <TextInput
          {...field}
          value={value[0] ?? ''}
          placeholder={element.placeholder?.text}
          maxLength={element.max_length ?? undefined}
          onChange={(e) => onChange(one(e.target.value))}
          dataQa={qa}
        />
      );
    case 'datepicker':
      return (
        <TextInput
          {...field}
          type="date"
          value={value[0] ?? ''}
          onChange={(e) => onChange(one(e.target.value))}
          dataQa={qa}
        />
      );
    case 'static_select':
      return (
        <Select {...field} value={value[0] ?? ''} onChange={(e) => onChange(one(e.target.value))} dataQa={qa}>
          <option value="">{element.placeholder?.text ?? t('blocks.select')}</option>
          {(element.options ?? []).map((option) => (
            <option key={option.value} value={option.value}>
              {option.text.text}
            </option>
          ))}
        </Select>
      );
    case 'users_select':
      return (
        <Combobox
          {...field}
          value={value[0] ?? ''}
          options={people}
          placeholder={element.placeholder?.text ?? t('blocks.selectPerson')}
          onChange={(next) => onChange(one(next))}
          dataQa={qa}
        />
      );
    case 'conversations_select':
      return (
        <Combobox
          {...field}
          value={value[0] ?? ''}
          options={channels}
          placeholder={element.placeholder?.text ?? t('blocks.selectConversation')}
          onChange={(next) => onChange(one(next))}
          dataQa={qa}
        />
      );
    case 'checkboxes':
      return (
        <Group label={field.label} optional={field.optional} hint={field.hint} error={error} qa={qa}>
          <div className="space-y-2">
            {(element.options ?? []).map((option) => {
              const id = option.value ?? '';
              return (
                <Checkbox
                  key={id}
                  label={option.text.text}
                  checked={value.includes(id)}
                  onChange={(on) => onChange(on ? [...value, id] : value.filter((v) => v !== id))}
                  dataQa={`${qa}-${id}`}
                />
              );
            })}
          </div>
        </Group>
      );
    case 'radio_buttons':
      return (
        <Group hint={field.hint} error={error} qa={qa}>
          <RadioCards
            label={field.optional ? `${field.label} ${tc('optional')}` : field.label}
            value={value[0] ?? ''}
            columns={1}
            options={(element.options ?? []).map((option) => ({
              value: option.value ?? '',
              label: option.text.text,
              dataQa: `${qa}-${option.value ?? ''}`,
            }))}
            onChange={(next) => onChange(one(next))}
          />
        </Group>
      );
    default:
      return null;
  }
}

function Group({
  label,
  optional,
  hint,
  error,
  qa,
  children,
}: {
  label?: string;
  optional?: boolean;
  hint?: string;
  error: string | null;
  qa: string;
  children: ReactNode;
}) {
  const { t } = useTranslation('common');
  const body = (
    <>
      {children}
      {error && (
        <p role="alert" className="mt-1.5 text-xs text-danger">
          {error}
        </p>
      )}
      {hint && !error && <p className="mt-1.5 text-xs text-muted">{hint}</p>}
    </>
  );
  if (!label) return <div data-qa={qa}>{body}</div>;
  return (
    <fieldset data-qa={qa}>
      <legend className="mb-1.5 text-sm font-medium text-fg-dim">
        {label}
        {optional && <span className="font-normal text-muted"> {t('optional')}</span>}
      </legend>
      {body}
    </fieldset>
  );
}
