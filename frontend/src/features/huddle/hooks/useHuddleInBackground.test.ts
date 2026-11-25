import { afterEach, describe, expect, it, vi } from 'vitest';
import { act, renderHook } from '@testing-library/react';
import { invoke } from '@tauri-apps/api/core';
import { useHuddleStore } from '@/stores/huddle';
import { useHuddleInBackground } from './useHuddleInBackground';

const phone = vi.hoisted(() => ({ android: true }));

vi.mock('@/lib/phoneApp', () => ({
  inPhoneApp: true,
  get inAndroidApp() {
    return phone.android;
  },
}));
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(async () => undefined) }));

const microphone = {} as MediaStream;

describe('a huddle in the background', () => {
  afterEach(() => {
    act(() => useHuddleStore.getState().setLocalStream(null));
    vi.mocked(invoke).mockClear();
    phone.android = true;
  });

  it('holds the microphone on Android from joining until leaving', () => {
    renderHook(() => useHuddleInBackground());
    expect(invoke).not.toHaveBeenCalled();

    act(() => useHuddleStore.getState().setLocalStream(microphone));
    expect(invoke).toHaveBeenCalledWith(
      'plugin:huddle|begin',
      expect.objectContaining({ title: 'In a huddle', text: 'Tap to go back to it.' }),
    );

    act(() => useHuddleStore.getState().setLocalStream({ ...microphone }));
    expect(invoke).toHaveBeenCalledTimes(1);

    act(() => useHuddleStore.getState().setLocalStream(null));
    expect(invoke).toHaveBeenLastCalledWith('plugin:huddle|end');
  });

  it('asks nothing of an iPhone', () => {
    phone.android = false;
    renderHook(() => useHuddleInBackground());

    act(() => useHuddleStore.getState().setLocalStream(microphone));

    expect(invoke).not.toHaveBeenCalled();
  });
});
