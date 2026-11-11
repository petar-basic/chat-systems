import { describe, it, expect, beforeEach } from 'vitest';
import { useDraftStore } from './drafts';
import { useInstanceStore, type InstanceConfig } from './instances';

function signIn(id: string) {
  const instance: InstanceConfig = {
    url: 'https://chat.example',
    user: { id, email: `${id}@chat.example`, display_name: id, avatar_url: null, instance_role: 'member' },
  };
  useInstanceStore.setState({ instances: [instance], activeInstanceUrl: instance.url });
}

describe('useDraftStore', () => {
  beforeEach(() => {
    localStorage.clear();
    useDraftStore.setState({ drafts: {} });
    signIn('ana');
  });

  it('stores and reads a draft by key', () => {
    useDraftStore.getState().setDraft('chan-1', 'hello');
    expect(useDraftStore.getState().getDraft('chan-1')).toBe('hello');
    expect(useDraftStore.getState().getDraft('other')).toBe('');
  });

  it('persists to localStorage', () => {
    useDraftStore.getState().setDraft('chan-1', 'persisted');
    expect(JSON.parse(localStorage.getItem('chat_drafts') || '{}')).toEqual({ 'ana:chan-1': 'persisted' });
  });

  it('deletes the draft when set to blank/whitespace', () => {
    useDraftStore.getState().setDraft('chan-1', 'something');
    useDraftStore.getState().setDraft('chan-1', '   ');
    expect(useDraftStore.getState().getDraft('chan-1')).toBe('');
    expect('chan-1' in useDraftStore.getState().drafts).toBe(false);
  });

  it('clearDraft removes the entry', () => {
    useDraftStore.getState().setDraft('chan-1', 'x');
    useDraftStore.getState().clearDraft('chan-1');
    expect(useDraftStore.getState().getDraft('chan-1')).toBe('');
  });

  it('shows nobody else who signs in on this browser what the last person was writing', () => {
    useDraftStore.getState().setDraft('chan-1', 'half a thought');

    signIn('ben');
    expect(useDraftStore.getState().getDraft('chan-1')).toBe('');
    useDraftStore.getState().setDraft('chan-1', 'mine');

    signIn('ana');
    expect(useDraftStore.getState().getDraft('chan-1')).toBe('half a thought');
  });
});
