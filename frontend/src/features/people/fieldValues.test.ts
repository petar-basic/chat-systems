import { describe, it, expect } from 'vitest';
import type { ProfileField } from './hooks/useProfileFields';
import {
  cardFields,
  changedValues,
  editableFields,
  filterParam,
  filledFields,
  formatFieldDate,
  moveItem,
  optionLabel,
  readFilters,
} from './fieldValues';

const field = (id: string, overrides: Partial<ProfileField> = {}): ProfileField => ({
  id,
  workspace_id: 'ws',
  label: id,
  kind: 'text',
  options: [],
  admins_only: false,
  on_card: false,
  position: 0,
  value_count: 0,
  ...overrides,
});

const department = field('dept', {
  kind: 'options',
  options: [
    { id: 'fin', label: 'Finance' },
    { id: 'sup', label: 'Support' },
  ],
  on_card: true,
});
const manager = field('mgr', { kind: 'person', on_card: true });
const office = field('office', { on_card: true });
const number = field('number', { admins_only: true });
const fields = [department, manager, office, number];

describe('readFilters and filterParam', () => {
  it('keep only options and person fields that exist', () => {
    expect(readFilters('dept:fin,office:x,gone:y,mgr:u1', fields)).toEqual({ dept: 'fin', mgr: 'u1' });
    expect(readFilters(null, fields)).toEqual({});
    expect(readFilters('nonsense', fields)).toEqual({});
  });

  it('write the pairs back, dropping a cleared filter', () => {
    expect(filterParam({ dept: 'fin', mgr: '' })).toBe('dept:fin');
    expect(filterParam({ dept: 'fin', mgr: 'u1' })).toBe('dept:fin,mgr:u1');
    expect(readFilters(filterParam({ dept: 'sup', mgr: 'u2' }), fields)).toEqual({ dept: 'sup', mgr: 'u2' });
  });
});

describe('which fields show', () => {
  const values = { dept: 'fin', mgr: 'u1', office: 'Belgrade', number: 'E-7' };

  it('shows the filled fields in the admin order', () => {
    expect(filledFields(fields, { office: 'Belgrade', dept: 'fin' }).map((e) => e.field.id)).toEqual([
      'dept',
      'office',
    ]);
    expect(filledFields(fields, undefined)).toEqual([]);
  });

  it('puts at most two on the hover card, skipping empty ones', () => {
    expect(cardFields(fields, values).map((e) => e.field.id)).toEqual(['dept', 'mgr']);
    expect(cardFields(fields, { office: 'Belgrade', number: 'E-7' }).map((e) => e.field.id)).toEqual([
      'office',
    ]);
  });

  it('lets a member edit everything but an admins-only field', () => {
    expect(editableFields(fields, false).map((f) => f.id)).toEqual(['dept', 'mgr', 'office']);
    expect(editableFields(fields, true)).toHaveLength(4);
  });

  it('names an option by its label', () => {
    expect(optionLabel(department, 'sup')).toBe('Support');
    expect(optionLabel(department, 'gone')).toBeNull();
  });
});

describe('changedValues', () => {
  it('sends only what changed, a blank as a clear', () => {
    expect(
      changedValues({ dept: 'fin', office: 'Belgrade' }, { dept: 'fin', office: '  ', mgr: 'u1' }, [
        department,
        manager,
        office,
      ]),
    ).toEqual({ office: null, mgr: 'u1' });
  });

  it('leaves out a field the person may not edit', () => {
    expect(changedValues({}, { number: 'E-8' }, editableFields(fields, false))).toEqual({});
  });
});

describe('formatFieldDate', () => {
  it('reads the day without shifting it across time zones', () => {
    expect(formatFieldDate('2026-02-03', 'en-GB')).toBe('3 Feb 2026');
    expect(formatFieldDate('not a date', 'en-GB')).toBe('not a date');
  });
});

describe('moveItem', () => {
  it('moves within bounds and ignores a move off the end', () => {
    expect(moveItem(['a', 'b', 'c'], 0, 2)).toEqual(['b', 'c', 'a']);
    const same = ['a', 'b'];
    expect(moveItem(same, 0, -1)).toBe(same);
    expect(moveItem(same, 1, 2)).toBe(same);
  });
});
