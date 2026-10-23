import { describe, it, expect } from 'vitest';
import {
  dateLabel,
  extractFilters,
  findMember,
  forTab,
  placeName,
  searchParams,
  setDate,
  type FilterContext,
} from './searchFilters';

const ctx: FilterContext = {
  members: [
    { user_id: 'u-me', display_name: 'Petar Basic' },
    { user_id: 'u-ana', display_name: 'Ana Jovanović' },
    { user_id: 'u-andrej', display_name: 'Andrej' },
    { user_id: 'u-lazar', display_name: 'Lazar' },
  ],
  channels: [
    { id: 'c-design', name: 'design', channel_type: 'public' },
    { id: 'c-ops', name: 'ops', channel_type: 'private' },
  ],
  conversations: [
    { id: 'd-lazar', kind: 'dm', participant_ids: ['u-me', 'u-lazar'] },
    { id: 'g-team', kind: 'group_dm', participant_ids: ['u-me', 'u-lazar', 'u-ana'] },
  ],
  currentUserId: 'u-me',
  today: new Date(2026, 9, 22, 15, 30),
};

describe('findMember', () => {
  it('finds a person by name, a unique beginning of it, or me', () => {
    expect(findMember('@anajovanović', ctx)).toBe('u-ana');
    expect(findMember('Lazar', ctx)).toBe('u-lazar');
    expect(findMember('@laz', ctx)).toBe('u-lazar');
    expect(findMember('me', ctx)).toBe('u-me');
  });

  it('refuses a beginning more than one person shares', () => {
    expect(findMember('@an', ctx)).toBeNull();
    expect(findMember('@nobody', ctx)).toBeNull();
  });
});

describe('extractFilters', () => {
  it('turns finished modifiers into filters and keeps the words', () => {
    expect(extractFilters('roadmap from:@ana in:#design has:files ', ctx, false)).toEqual({
      text: 'roadmap ',
      filters: { from: 'u-ana', in: 'c-design', has: 'file' },
    });
  });

  it('waits for the modifier being typed until a space or Enter', () => {
    expect(extractFilters('roadmap from:@an', ctx, false)).toBeNull();
    expect(extractFilters('roadmap in:#design', ctx, false)).toBeNull();
    expect(extractFilters('roadmap in:#design', ctx, true)).toEqual({
      text: 'roadmap',
      filters: { in: 'c-design' },
    });
  });

  it('reads a file type, singular or plural', () => {
    expect(extractFilters('budget type:spreadsheets ', ctx, false)).toEqual({
      text: 'budget ',
      filters: { kind: 'spreadsheet' },
    });
    expect(extractFilters('type:PDF', ctx, true)).toEqual({ text: '', filters: { kind: 'pdf' } });
  });

  it('leaves what it cannot place as words', () => {
    expect(extractFilters('in:#nowhere has:gif ', ctx, false)).toBeNull();
    expect(extractFilters('type:gif ', ctx, false)).toBeNull();
    expect(extractFilters('in:#nowhere from:me ', ctx, false)).toEqual({
      text: 'in:#nowhere ',
      filters: { from: 'u-me' },
    });
  });

  it('reads in:@person as the DM with them', () => {
    expect(extractFilters('in:@lazar ', ctx, false)?.filters).toEqual({ in: 'd-lazar' });
    expect(extractFilters('in:@ana ', ctx, false)).toBeNull();
  });

  it('reads dates, today and yesterday', () => {
    expect(extractFilters('after:2026-10-01 before:2026-10-10', ctx, true)?.filters).toEqual({
      after: '2026-10-01',
      before: '2026-10-10',
      on: undefined,
    });
    expect(extractFilters('on:yesterday', ctx, true)?.filters).toMatchObject({ on: '2026-10-21' });
    expect(extractFilters('on:today', ctx, true)?.filters).toMatchObject({ on: '2026-10-22' });
    expect(extractFilters('before:2026-02-30', ctx, true)).toBeNull();
  });
});

describe('searchParams', () => {
  const local = (y: number, m: number, d: number) => new Date(y, m - 1, d).toISOString();

  it('maps people, places and kinds to the API', () => {
    expect(searchParams({ from: 'u-ana', in: 'c-design', has: 'link' })).toEqual({
      user_id: 'u-ana',
      channel_id: 'c-design',
      has: 'link',
    });
  });

  it('keeps has: to messages and type: to files', () => {
    const filters = { from: 'u-ana', has: 'file' as const, kind: 'pdf' as const };
    expect(searchParams(forTab(filters, 'messages'))).toEqual({ user_id: 'u-ana', has: 'file' });
    expect(searchParams(forTab(filters, 'files'))).toEqual({ user_id: 'u-ana', kind: 'pdf' });
  });

  it('reads after and before as Slack does, both excluding the day named', () => {
    expect(searchParams({ after: '2026-10-01', before: '2026-10-10' })).toEqual({
      after: local(2026, 10, 2),
      before: local(2026, 10, 10),
    });
  });

  it('turns on into that whole local day', () => {
    expect(searchParams({ on: '2026-10-31' })).toEqual({
      after: local(2026, 10, 31),
      before: local(2026, 11, 1),
    });
  });
});

describe('date filters', () => {
  it('lets on and a range replace each other', () => {
    const on = setDate({ has: 'file', after: '2026-10-01' }, { on: '2026-10-05' });
    expect(on).toEqual({ has: 'file', on: '2026-10-05', after: undefined, before: undefined });
    expect(setDate(on, { after: '2026-10-01' })).toEqual({
      has: 'file',
      on: undefined,
      after: '2026-10-01',
    });
  });

  it('describes the date chip', () => {
    expect(dateLabel({})).toBeNull();
    expect(dateLabel({ on: '2026-10-05' })).toMatch(/^On /);
    expect(dateLabel({ after: '2026-10-01', before: '2026-10-10' })).toMatch(/^After .+, before /);
    expect(dateLabel({ before: '2026-10-10' })).toMatch(/^Before /);
  });
});

describe('placeName', () => {
  it('names channels and conversations', () => {
    expect(placeName(ctx, 'c-design')).toBe('#design');
    expect(placeName(ctx, 'd-lazar')).toBe('Lazar');
    expect(placeName(ctx, 'g-team')).toBe('Lazar, Ana Jovanović');
  });
});
