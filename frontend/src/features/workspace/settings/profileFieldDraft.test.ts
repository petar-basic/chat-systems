import { describe, it, expect } from 'vitest';
import type { ProfileField } from '@/features/people';
import { fieldDraft, fieldRequest, readyToSave } from './profileFieldDraft';

const department: ProfileField = {
  id: 'dept',
  workspace_id: 'ws',
  label: 'Department',
  hint: 'Where you sit in the org chart',
  kind: 'options',
  options: [
    { id: 'fin', label: 'Finance' },
    { id: 'sup', label: 'Support' },
  ],
  admins_only: false,
  on_card: true,
  position: 0,
  value_count: 3,
};

describe('fieldDraft', () => {
  it('starts a new field as text with two empty options ready', () => {
    const draft = fieldDraft(null);
    expect(draft.kind).toBe('text');
    expect(draft.options).toHaveLength(2);
    expect(readyToSave(draft)).toBe(false);
  });

  it('copies an existing field, keeping option ids', () => {
    const draft = fieldDraft(department);
    expect(draft.label).toBe('Department');
    expect(draft.options.map((o) => o.id)).toEqual(['fin', 'sup']);
    expect(draft.onCard).toBe(true);
  });
});

describe('readyToSave', () => {
  it('needs a label', () => {
    expect(readyToSave({ ...fieldDraft(null), label: '  ' })).toBe(false);
    expect(readyToSave({ ...fieldDraft(null), label: 'Office' })).toBe(true);
  });

  it('needs an options field to have distinct options', () => {
    const draft = { ...fieldDraft(null), label: 'Level', kind: 'options' as const };
    expect(readyToSave(draft)).toBe(false);
    const named = (labels: string[]) => ({
      ...draft,
      options: labels.map((label, i) => ({ key: String(i), label })),
    });
    expect(readyToSave(named(['Junior', '', 'Senior']))).toBe(true);
    expect(readyToSave(named(['Junior', 'junior ']))).toBe(false);
  });
});

describe('fieldRequest', () => {
  it('sends trimmed options with their ids and drops blank rows', () => {
    const draft = fieldDraft(department);
    draft.options = [
      { ...draft.options[1], label: ' Customer support ' },
      { key: 'new', label: 'Legal' },
      { key: 'blank', label: '   ' },
    ];
    expect(fieldRequest(draft).options).toEqual([
      { id: 'sup', label: 'Customer support' },
      { id: null, label: 'Legal' },
    ]);
  });

  it('sends no options for another kind', () => {
    const draft = { ...fieldDraft(null), label: ' Office ', hint: ' Building and floor ' };
    expect(fieldRequest(draft)).toEqual({
      label: 'Office',
      hint: 'Building and floor',
      options: [],
      admins_only: false,
      on_card: false,
    });
  });
});
