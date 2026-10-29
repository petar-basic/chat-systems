import { beforeEach, describe, expect, it } from 'vitest';
import { applyCommandResponse } from './commandAnswers';
import { useCommandAnswerStore } from '@/stores/commandAnswer';

const frame = (response_type: string, text: string, channel_id = 'ch-1') => ({
  type: 'command.response' as const,
  channel_id,
  command: 'deploy',
  response_type,
  text,
});

describe('a command that answers later', () => {
  beforeEach(() => {
    useCommandAnswerStore.setState({ answer: null });
  });

  it('replaces the pending line with the answer', () => {
    useCommandAnswerStore
      .getState()
      .show({ channelId: 'ch-1', text: '/deploy is taking a moment…', pending: true });
    applyCommandResponse(frame('ephemeral', 'deployed'));
    expect(useCommandAnswerStore.getState().answer).toEqual({
      channelId: 'ch-1',
      text: 'deployed',
      pending: false,
    });
  });

  it('shows a final error the same way', () => {
    useCommandAnswerStore
      .getState()
      .show({ channelId: 'ch-1', text: '/deploy is taking a moment…', pending: true });
    applyCommandResponse(frame('error', '/deploy could not be reached'));
    expect(useCommandAnswerStore.getState().answer?.text).toBe('/deploy could not be reached');
  });

  it('clears the pending line once the answer is posted in the channel', () => {
    useCommandAnswerStore
      .getState()
      .show({ channelId: 'ch-1', text: '/deploy is working on it…', pending: true });
    applyCommandResponse(frame('in_channel', ''));
    expect(useCommandAnswerStore.getState().answer).toBeNull();
  });

  it('leaves another channel and an earlier answer alone', () => {
    useCommandAnswerStore
      .getState()
      .show({ channelId: 'ch-2', text: '/deploy is working on it…', pending: true });
    applyCommandResponse(frame('in_channel', '', 'ch-1'));
    expect(useCommandAnswerStore.getState().answer?.channelId).toBe('ch-2');

    useCommandAnswerStore.getState().show({ channelId: 'ch-1', text: 'done', pending: false });
    applyCommandResponse(frame('in_channel', ''));
    expect(useCommandAnswerStore.getState().answer?.text).toBe('done');
  });
});
