import { describe, it, expect } from 'vitest';
import { fragmentRange } from './fold';

describe('fragmentRange', () => {
  it('finds a fragment ignoring case and diacritics, in the original text', () => {
    expect(fragmentRange('Q3 budget.xlsx', 'BUDG')).toEqual([3, 7]);
    expect(fragmentRange('Izveštaj.pdf', 'vesta')).toEqual([2, 7]);
    expect(fragmentRange('Izveštaj.pdf', ' ')).toBeNull();
    expect(fragmentRange('Izveštaj.pdf', 'xlsx')).toBeNull();
  });
});
