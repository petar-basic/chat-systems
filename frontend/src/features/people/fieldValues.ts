import type { ProfileField } from './hooks/useProfileFields';

export type FieldValues = Record<string, string>;
export type Filters = Record<string, string>;

export const MAX_ON_CARD = 2;
export const MAX_FIELDS = 50;

export interface FieldEntry {
  field: ProfileField;
  value: string;
}

export function filledFields(fields: ProfileField[], values: FieldValues | undefined): FieldEntry[] {
  if (!values) return [];
  return fields.flatMap((field) => {
    const value = values[field.id];
    return value ? [{ field, value }] : [];
  });
}

export function cardFields(fields: ProfileField[], values: FieldValues | undefined): FieldEntry[] {
  return filledFields(
    fields.filter((f) => f.on_card),
    values,
  ).slice(0, MAX_ON_CARD);
}

export function editableFields(fields: ProfileField[], isAdmin: boolean): ProfileField[] {
  return fields.filter((f) => isAdmin || !f.admins_only);
}

export function filterableFields(fields: ProfileField[]): ProfileField[] {
  return fields.filter((f) => f.kind === 'options' || f.kind === 'person');
}

export function optionLabel(field: ProfileField, value: string): string | null {
  return field.options.find((o) => o.id === value)?.label ?? null;
}

export function formatFieldDate(value: string, locale?: string): string {
  const [year, month, day] = value.split('-').map(Number);
  if (!year || !month || !day) return value;
  return new Date(Date.UTC(year, month - 1, day)).toLocaleDateString(locale, {
    dateStyle: 'medium',
    timeZone: 'UTC',
  });
}

export function changedValues(
  saved: FieldValues,
  draft: FieldValues,
  fields: ProfileField[],
): Record<string, string | null> {
  const changes: Record<string, string | null> = {};
  for (const field of fields) {
    const before = saved[field.id] ?? '';
    const after = (draft[field.id] ?? '').trim();
    if (before !== after) changes[field.id] = after || null;
  }
  return changes;
}

export function readFilters(param: string | null, fields: ProfileField[]): Filters {
  const known = new Set(filterableFields(fields).map((f) => f.id));
  const filters: Filters = {};
  for (const pair of (param ?? '').split(',')) {
    const [fieldId, value] = pair.split(':');
    if (fieldId && value && known.has(fieldId)) filters[fieldId] = value;
  }
  return filters;
}

export function filterParam(filters: Filters): string {
  return Object.entries(filters)
    .filter(([, value]) => value)
    .map(([fieldId, value]) => `${fieldId}:${value}`)
    .join(',');
}

export function moveItem<T>(items: T[], from: number, to: number): T[] {
  if (from === to || to < 0 || to >= items.length) return items;
  const next = [...items];
  const [moved] = next.splice(from, 1);
  next.splice(to, 0, moved);
  return next;
}
