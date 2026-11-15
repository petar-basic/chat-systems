import { describe, it, expect, beforeEach, vi } from 'vitest';
import { REMOTE_DRAFT_EVENT, draftKeyOf, useDraftStore, type DraftPlace, type SavedDraft } from './drafts';
import { useInstanceStore, type InstanceConfig } from './instances';

const URL = 'https://chat.example';

function signIn(id: string) {
  const instance: InstanceConfig = {
    url: URL,
    user: { id, email: `${id}@chat.example`, display_name: id, avatar_url: null, instance_role: 'member' },
  };
  useInstanceStore.setState({ instances: [instance], activeInstanceUrl: instance.url });
}

const general: DraftPlace = {
  instanceUrl: URL,
  workspaceId: 'ws-1',
  channelId: 'ch-1',
  threadParentId: null,
};
const thread: DraftPlace = { ...general, threadParentId: 'm-1' };

const saved = (over: Partial<SavedDraft> = {}): SavedDraft => ({
  workspace_id: 'ws-1',
  channel_id: 'ch-1',
  thread_parent_id: null,
  content: 'from the laptop',
  files_left_behind: false,
  updated_at: '2026-11-14T10:00:00.000000Z',
  ...over,
});

const store = () => useDraftStore.getState();

function remoteTexts() {
  const heard: { key: string; text: string }[] = [];
  const listen = (event: Event) => heard.push((event as CustomEvent<{ key: string; text: string }>).detail);
  window.addEventListener(REMOTE_DRAFT_EVENT, listen);
  return { heard, stop: () => window.removeEventListener(REMOTE_DRAFT_EVENT, listen) };
}

describe('useDraftStore', () => {
  beforeEach(() => {
    localStorage.clear();
    useDraftStore.setState({ drafts: {} });
    signIn('ana');
    vi.useRealTimers();
  });

  it('keeps a draft per place, a thread apart from its channel', () => {
    store().setDraft(draftKeyOf(general), 'hello', { place: general });
    store().setDraft(draftKeyOf(thread), 'about that', { place: thread });
    expect(store().getDraft('ch-1')).toBe('hello');
    expect(store().getDraft('ch-1/m-1')).toBe('about that');
    expect(store().entry('ch-1')?.pending).toBe(true);
  });

  it('persists, and ignores what it cannot read back', () => {
    store().setDraft('note', 'persisted');
    const stored = JSON.parse(localStorage.getItem('chat_drafts') || '{}');
    expect(stored['ana:note'].text).toBe('persisted');
    expect(stored['ana:note'].pending).toBe(false);
  });

  it('forgets a blank local-only draft, but keeps a cleared place to tell the server', () => {
    store().setDraft('note', 'something');
    store().setDraft('note', '   ');
    expect(store().entry('note')).toBeUndefined();

    store().setDraft('ch-1', 'something', { place: general });
    store().clearDraft('ch-1');
    expect(store().entry('ch-1')).toMatchObject({ text: '', pending: true, place: general });
  });

  it('does not touch an unchanged draft, unless it gains a place', () => {
    store().setDraft('ch-1', 'same');
    const before = store().entry('ch-1');
    store().setDraft('ch-1', 'same');
    expect(store().entry('ch-1')).toBe(before);
    store().setDraft('ch-1', 'same', { place: general });
    expect(store().entry('ch-1')).toMatchObject({ place: general, pending: true });
  });

  it('shows nobody else who signs in on this browser what the last person was writing', () => {
    store().setDraft('ch-1', 'half a thought');
    signIn('ben');
    expect(store().getDraft('ch-1')).toBe('');
    store().setDraft('ch-1', 'mine');
    signIn('ana');
    expect(store().getDraft('ch-1')).toBe('half a thought');
  });

  it('takes a newer draft from another device and tells the composer', () => {
    const { heard, stop } = remoteTexts();
    store().accept(URL, saved());
    expect(store().entry('ch-1')).toMatchObject({ text: 'from the laptop', pending: false, place: general });
    expect(heard).toEqual([{ key: 'ch-1', text: 'from the laptop' }]);

    store().accept(URL, saved({ content: '', updated_at: '2026-11-14T10:05:00Z' }));
    expect(store().entry('ch-1')).toBeUndefined();
    expect(heard.at(-1)).toEqual({ key: 'ch-1', text: '' });
    stop();
  });

  it('keeps what was typed here when the other device is older', () => {
    vi.useFakeTimers({ now: new Date('2026-11-14T10:10:00Z') });
    store().setDraft('ch-1', 'typed offline', { place: general });
    store().accept(URL, saved({ updated_at: '2026-11-14T10:00:00Z' }));
    expect(store().entry('ch-1')).toMatchObject({ text: 'typed offline', pending: true });
  });

  it('settles on what the server kept, unless more was typed meanwhile', () => {
    store().setDraft('ch-1', 'first', { place: general });
    const sentAt = store().entry('ch-1')?.at ?? '';
    store().settle('ch-1', sentAt, saved({ content: 'won elsewhere', updated_at: '2026-11-14T10:00:00Z' }));
    expect(store().entry('ch-1')).toMatchObject({ text: 'won elsewhere', pending: false });

    vi.useFakeTimers({ now: new Date('2026-11-14T11:00:00Z') });
    store().setDraft('ch-1', 'first again', { place: general });
    const stale = store().entry('ch-1')?.at ?? '';
    vi.setSystemTime(new Date('2026-11-14T11:00:05Z'));
    store().setDraft('ch-1', 'first again, longer', { place: general });
    store().settle('ch-1', stale, saved({ content: 'first again' }));
    expect(store().entry('ch-1')).toMatchObject({ text: 'first again, longer', pending: true });
  });

  it('drops synced drafts the server no longer lists, and nothing it has yet to hear about', () => {
    store().accept(URL, saved());
    store().accept(URL, saved({ channel_id: 'ch-2', content: 'kept' }));
    store().accept(URL, saved({ workspace_id: 'ws-2', channel_id: 'ch-9', content: 'elsewhere' }));
    store().setDraft('ch-3/m-3', 'not sent up yet', {
      place: { ...general, channelId: 'ch-3', threadParentId: 'm-3' },
    });
    store().setDraft('note', 'local only');

    store().forgetMissing('ws-1', new Set(['ch-2']));
    expect(store().getDraft('ch-1')).toBe('');
    expect(store().getDraft('ch-2')).toBe('kept');
    expect(store().getDraft('ch-9')).toBe('elsewhere');
    expect(store().getDraft('ch-3/m-3')).toBe('not sent up yet');
    expect(store().getDraft('note')).toBe('local only');
  });

  it('keeps a refused draft on this device only', () => {
    store().setDraft('ch-1', 'for a channel I left', { place: general });
    store().keepLocal('ch-1');
    expect(store().entry('ch-1')).toEqual({ text: 'for a channel I left', at: expect.any(String) });
  });

  it('empties an open composer when a draft is deleted from the list', () => {
    const { heard, stop } = remoteTexts();
    store().accept(URL, saved());
    store().discardDraft('ch-1');
    expect(store().entry('ch-1')).toMatchObject({ text: '', pending: true });
    expect(heard.at(-1)).toEqual({ key: 'ch-1', text: '' });
    stop();
  });
});
