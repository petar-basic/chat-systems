import type { ProfileField, ProfileFieldKind } from '@/features/people';

export interface OptionDraft {
  key: string;
  id?: string;
  label: string;
}

export interface FieldDraft {
  label: string;
  hint: string;
  kind: ProfileFieldKind;
  options: OptionDraft[];
  adminsOnly: boolean;
  onCard: boolean;
}

export function fieldDraft(field: ProfileField | null): FieldDraft {
  if (!field) {
    return {
      label: '',
      hint: '',
      kind: 'text',
      options: [
        { key: crypto.randomUUID(), label: '' },
        { key: crypto.randomUUID(), label: '' },
      ],
      adminsOnly: false,
      onCard: false,
    };
  }
  return {
    label: field.label,
    hint: field.hint ?? '',
    kind: field.kind,
    options: field.options.map((o) => ({ key: o.id, id: o.id, label: o.label })),
    adminsOnly: field.admins_only,
    onCard: field.on_card,
  };
}

export function filledOptions(draft: FieldDraft): OptionDraft[] {
  return draft.options.filter((o) => o.label.trim());
}

export function readyToSave(draft: FieldDraft): boolean {
  if (!draft.label.trim()) return false;
  if (draft.kind !== 'options') return true;
  const labels = filledOptions(draft).map((o) => o.label.trim().toLowerCase());
  return labels.length > 0 && new Set(labels).size === labels.length;
}

export interface FieldRequest {
  label: string;
  hint: string;
  options: { id: string | null; label: string }[];
  admins_only: boolean;
  on_card: boolean;
}

export function fieldRequest(draft: FieldDraft): FieldRequest {
  return {
    label: draft.label.trim(),
    hint: draft.hint.trim(),
    options:
      draft.kind === 'options'
        ? filledOptions(draft).map((o) => ({ id: o.id ?? null, label: o.label.trim() }))
        : [],
    admins_only: draft.adminsOnly,
    on_card: draft.onCard,
  };
}
