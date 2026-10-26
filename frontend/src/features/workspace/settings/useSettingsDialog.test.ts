import { describe, it, expect, afterEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { useSettingsDialog } from './useSettingsDialog';

describe('useSettingsDialog', () => {
  afterEach(() => {
    window.history.replaceState({}, '', '/');
  });

  it('opens the section a link names and takes it out of the address', () => {
    window.history.replaceState({}, '', '/app/ws-1?settings=audit-log&x=1');
    const { result } = renderHook(() => useSettingsDialog());
    expect(result.current.dialog).toEqual({ section: 'audit-log', inviting: false });
    expect(window.location.search).toBe('?x=1');
  });

  it('opens General for a section it does not know', () => {
    window.history.replaceState({}, '', '/app/ws-1?settings=nope');
    const { result } = renderHook(() => useSettingsDialog());
    expect(result.current.dialog?.section).toBe('general');
  });

  it('stays closed without a link, and opens and closes on request', () => {
    const { result } = renderHook(() => useSettingsDialog());
    expect(result.current.dialog).toBeNull();
    act(() => result.current.open('members', true));
    expect(result.current.dialog).toEqual({ section: 'members', inviting: true });
    act(() => result.current.close());
    expect(result.current.dialog).toBeNull();
  });
});
