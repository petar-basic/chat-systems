import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, renderHook } from '@testing-library/react';
import { ANSWER_WAIT_MS, useCommandAnswerStore } from '@/stores/commandAnswer';
import { useSlashCommand } from './useSlashCommand';

const runCommand = vi.hoisted(() => vi.fn());

vi.mock('@/lib/slashCommands', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@/lib/slashCommands')>()),
  runCommand,
}));

describe('useSlashCommand', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    runCommand.mockReset();
    useCommandAnswerStore.setState({ answers: {} });
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('stops waiting for an app that never answers once its response url has expired', async () => {
    runCommand.mockResolvedValue({ response_type: 'pending', text: '/deploy is taking a moment…' });
    const { result } = renderHook(() => useSlashCommand('ch-1', undefined, undefined));

    await act(() => result.current.run('/deploy prod'));
    expect(result.current).toEqual(expect.objectContaining({ pending: true }));

    act(() => vi.advanceTimersByTime(ANSWER_WAIT_MS - 1000));
    expect(result.current.pending).toBe(true);

    act(() => vi.advanceTimersByTime(1000));
    expect(result.current).toEqual(
      expect.objectContaining({ answer: 'No answer from /deploy', pending: false }),
    );
  });

  it('puts a final answer away after a while', async () => {
    runCommand.mockResolvedValue({ response_type: 'ephemeral', text: 'You are away' });
    const { result } = renderHook(() => useSlashCommand('ch-1', undefined, undefined));

    await act(() => result.current.run('/away'));
    expect(result.current.answer).toBe('You are away');

    act(() => vi.advanceTimersByTime(10_000));
    expect(result.current.answer).toBeNull();
  });

  it('keeps the answer of a thread apart from the one of its channel', async () => {
    runCommand.mockResolvedValue({ response_type: 'ephemeral', text: 'You are away' });
    const channel = renderHook(() => useSlashCommand('ch-1', undefined, undefined));
    const thread = renderHook(() => useSlashCommand('ch-1', 'root-1', 'https://chat.example'));

    await act(() => thread.result.current.run('/away'));

    expect(runCommand).toHaveBeenCalledWith('ch-1', '/away', 'https://chat.example', 'root-1');
    expect(thread.result.current.answer).toBe('You are away');
    expect(channel.result.current.answer).toBeNull();
  });

  it('leaves a message that is not a command to be sent', async () => {
    const { result } = renderHook(() => useSlashCommand('ch-1', undefined, undefined));

    let ran = true;
    await act(async () => {
      ran = await result.current.run('hello /there');
    });

    expect(ran).toBe(false);
    expect(runCommand).not.toHaveBeenCalled();
  });
});
