import { useMemo, useState } from 'react';
import { useSearchParams } from 'react-router';
import { useTranslation } from 'react-i18next';
import { Menu, Search, Users } from 'lucide-react';
import { IconButton } from '@/shared/components/IconButton/IconButton';
import { TextInput } from '@/shared/components/Form/TextInput';
import { Select } from '@/shared/components/Form/Select';
import { Combobox, type ComboboxOption } from '@/shared/components/Form/Combobox';
import { Avatar } from '@/shared/components/Avatar/Avatar';
import { Button } from '@/shared/components/Button/Button';
import { useDebouncedValue } from '@/shared/hooks/useDebouncedValue';
import { useProfilePanel } from '@/stores/profilePanel';
import { displayNameOf } from '@/lib/userHelpers';
import type { WorkspaceMember } from '@/stores/workspace';
import { useDirectory, useProfileFields, type ProfileField } from './hooks/useProfileFields';
import { cardFields, filterableFields, filterParam, readFilters, type Filters } from './fieldValues';
import { FieldList } from './FieldValue';

interface Props {
  workspaceId: string;
  instanceUrl?: string;
  members: WorkspaceMember[];
  onOpenNav?: () => void;
}

export function PeopleView({ workspaceId, instanceUrl, members, onOpenNav }: Props) {
  const { t } = useTranslation('profile');
  const [params, setParams] = useSearchParams();
  const { fields, isSuccess: fieldsReady } = useProfileFields(workspaceId, instanceUrl);
  const [typed, setTyped] = useState(params.get('q') ?? '');
  const q = useDebouncedValue(typed.trim(), 250);
  const filters = useMemo(() => readFilters(params.get('filter'), fields), [params, fields]);
  const filter = filterParam(filters);
  const directory = useDirectory(workspaceId, q, filter, instanceUrl, fieldsReady);
  const show = useProfilePanel((s) => s.show);
  const people = (directory.data ?? []).filter((m) => !m.is_bot);
  const filtering = !!q || !!filter;

  const update = (next: { q?: string; filters?: Filters }) => {
    const nextParams = new URLSearchParams(params);
    const nextQ = next.q ?? params.get('q') ?? '';
    const nextFilter = next.filters ? filterParam(next.filters) : filter;
    if (nextQ) nextParams.set('q', nextQ);
    else nextParams.delete('q');
    if (nextFilter) nextParams.set('filter', nextFilter);
    else nextParams.delete('filter');
    setParams(nextParams, { replace: true });
  };

  const setFilter = (fieldId: string, value: string) => update({ filters: { ...filters, [fieldId]: value } });

  const clear = () => {
    setTyped('');
    update({ q: '', filters: {} });
  };

  return (
    <main className="flex-1 flex flex-col min-w-0" aria-label={t('people.title')} data-qa="people">
      <div className="h-14 px-4 flex items-center gap-2 border-b border-line/50 bg-surface/30 shrink-0">
        {onOpenNav && (
          <IconButton
            label={t('header.openNavigation', { ns: 'channel' })}
            icon={<Menu className="w-5 h-5" />}
            onClick={onOpenNav}
            size="md"
            className="lg:hidden -ml-1"
          />
        )}
        <h1 className="font-semibold text-fg">{t('people.title')}</h1>
      </div>

      <div className="px-4 py-3 border-b border-line/50 space-y-3 shrink-0" data-qa="people-filters">
        <TextInput
          label={t('people.search')}
          labelHidden
          value={typed}
          onChange={(e) => {
            setTyped(e.target.value);
            update({ q: e.target.value.trim() });
          }}
          placeholder={t('people.searchPlaceholder')}
          leading={<Search className="w-4 h-4" />}
          dataQa="people-search"
        />
        {filterableFields(fields).length > 0 && (
          <div className="flex flex-wrap gap-3">
            {filterableFields(fields).map((field) => (
              <FilterControl
                key={field.id}
                field={field}
                value={filters[field.id] ?? ''}
                members={members}
                onChange={(value) => setFilter(field.id, value)}
              />
            ))}
          </div>
        )}
      </div>

      <div className="flex-1 overflow-y-auto px-4 py-3">
        {directory.isError && <p className="text-sm text-danger">{t('people.failed')}</p>}
        {directory.isSuccess && people.length === 0 && (
          <div className="flex flex-col items-center justify-center py-16 text-center" data-qa="people-empty">
            <Users className="w-10 h-10 text-muted mb-3" />
            <p className="text-fg-soft font-medium">{t(filtering ? 'people.noMatch' : 'people.empty')}</p>
            {filtering && (
              <Button variant="ghost" size="sm" className="mt-2" onClick={clear} data-qa="people-clear">
                {t('people.clear')}
              </Button>
            )}
          </div>
        )}
        <ul className="grid gap-2 sm:grid-cols-2 xl:grid-cols-3" data-qa="people-list">
          {people.map((person) => (
            <PersonCard
              key={person.user_id}
              person={person}
              fields={fields}
              onOpen={() => show(person.user_id)}
            />
          ))}
        </ul>
      </div>
    </main>
  );
}

function PersonCard({
  person,
  fields,
  onOpen,
}: {
  person: WorkspaceMember;
  fields: ProfileField[];
  onOpen: () => void;
}) {
  const name = displayNameOf(person.display_name || person.email);
  return (
    <li data-qa="people-row" className="rounded-lg border border-line/60 bg-surface/40">
      <button
        type="button"
        onClick={onOpen}
        className="w-full flex items-start gap-3 p-3 text-left cursor-pointer hover:bg-raised/30 rounded-lg"
      >
        <Avatar userId={person.user_id} name={name} avatarUrl={person.avatar_url} size="lg" />
        <span className="min-w-0 flex-1">
          <span className="block text-sm font-semibold text-fg truncate">{name}</span>
          {person.title && <span className="block text-xs text-fg-dim truncate">{person.title}</span>}
        </span>
      </button>
      <FieldList
        entries={cardFields(fields, person.profile_fields)}
        dataQa="people-row-fields"
        className="px-3 pb-3 -mt-1"
      />
    </li>
  );
}

function FilterControl({
  field,
  value,
  members,
  onChange,
}: {
  field: ProfileField;
  value: string;
  members: WorkspaceMember[];
  onChange: (value: string) => void;
}) {
  const { t } = useTranslation('profile');
  const people = useMemo<ComboboxOption[]>(
    () => [
      { value: '', label: t('people.anyone') },
      ...members
        .filter((m) => !m.is_bot)
        .map((m) => ({ value: m.user_id, label: displayNameOf(m.display_name || m.email) }))
        .sort((a, b) => a.label.localeCompare(b.label)),
    ],
    [members, t],
  );

  if (field.kind === 'person') {
    return (
      <Combobox
        label={field.label}
        value={value}
        options={people}
        onChange={onChange}
        className="w-56"
        dataQa={`people-filter-${field.id}`}
      />
    );
  }
  return (
    <Select
      label={field.label}
      value={value}
      onChange={(e) => onChange(e.target.value)}
      className="w-56"
      dataQa={`people-filter-${field.id}`}
    >
      <option value="">{t('people.any')}</option>
      {field.options.map((option) => (
        <option key={option.id} value={option.id}>
          {option.label}
        </option>
      ))}
    </Select>
  );
}
