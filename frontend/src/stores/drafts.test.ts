import { describe, it, expect, beforeEach } from 'vitest';
import { useDraftStore } from './drafts';

describe('useDraftStore', () => {
  beforeEach(() => {
    localStorage.clear();
    useDraftStore.setState({ drafts: {} });
  });

  it('stores and reads a draft by key', () => {
    useDraftStore.getState().setDraft('chan-1', 'hello');
    expect(useDraftStore.getState().getDraft('chan-1')).toBe('hello');
    expect(useDraftStore.getState().getDraft('other')).toBe('');
  });

  it('persists to localStorage', () => {
    useDraftStore.getState().setDraft('chan-1', 'persisted');
    expect(JSON.parse(localStorage.getItem('chat_drafts') || '{}')).toEqual({ 'chan-1': 'persisted' });
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
});
