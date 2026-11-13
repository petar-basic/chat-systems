import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { TextInput } from '@/shared/components/Form/TextInput';
import { Select } from '@/shared/components/Form/Select';
import { Combobox, type ComboboxOption } from '@/shared/components/Form/Combobox';
import { useUserCache } from '@/stores/users';
import { displayNameOf } from '@/lib/userHelpers';
import type { ProfileField } from './hooks/useProfileFields';
import type { FieldValues } from './fieldValues';

interface Props {
  fields: ProfileField[];
  values: FieldValues;
  onChange: (fieldId: string, value: string) => void;
}

export function ProfileFieldInputs({ fields, values, onChange }: Props) {
  const { t } = useTranslation('profile');
  const users = useUserCache((s) => s.users);
  const people = useMemo<ComboboxOption[]>(
    () => [
      { value: '', label: t('fields.nobody') },
      ...[...users.values()]
        .filter((u) => !u.left && !u.is_bot)
        .map((u) => ({
          value: u.id,
          label: displayNameOf(u.display_name || u.email),
          detail: u.title ?? undefined,
        }))
        .sort((a, b) => a.label.localeCompare(b.label)),
    ],
    [users, t],
  );

  return (
    <div className="space-y-5" data-qa="profile-fields-inputs">
      {fields.map((field) => {
        const value = values[field.id] ?? '';
        const common = {
          label: field.label,
          optional: true,
          hint: field.hint ?? undefined,
          dataQa: `profile-field-${field.id}`,
        };
        switch (field.kind) {
          case 'options':
            return (
              <Select
                key={field.id}
                {...common}
                value={value}
                onChange={(e) => onChange(field.id, e.target.value)}
              >
                <option value="">{t('fields.none')}</option>
                {field.options.map((option) => (
                  <option key={option.id} value={option.id}>
                    {option.label}
                  </option>
                ))}
              </Select>
            );
          case 'person':
            return (
              <Combobox
                key={field.id}
                {...common}
                value={value}
                options={people}
                onChange={(next) => onChange(field.id, next)}
                placeholder={t('fields.pickPerson')}
              />
            );
          case 'date':
            return (
              <TextInput
                key={field.id}
                {...common}
                type="date"
                value={value}
                onChange={(e) => onChange(field.id, e.target.value)}
              />
            );
          case 'link':
            return (
              <TextInput
                key={field.id}
                {...common}
                type="url"
                value={value}
                onChange={(e) => onChange(field.id, e.target.value)}
                placeholder="https://"
                maxLength={2000}
              />
            );
          default:
            return (
              <TextInput
                key={field.id}
                {...common}
                value={value}
                onChange={(e) => onChange(field.id, e.target.value)}
                maxLength={200}
              />
            );
        }
      })}
    </div>
  );
}
