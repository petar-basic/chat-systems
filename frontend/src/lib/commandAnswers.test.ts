import { beforeEach, describe, expect, it } from 'vitest';
import { applyCommandResponse, giveUpOnAnswers } from './commandAnswers';
import { answerPlace, useCommandAnswerStore } from '@/stores/commandAnswer';

const frame = (
  response_type: string,
  text: string,
  channel_id = 'ch-1',
  thread_parent_id: string | null = null,
) => ({
  type: 'command.response' as const,
  channel_id,
  thread_parent_id,
  command: 'deploy',
  response_type,
  text,
});

const waiting = (place: string, text = '/deploy is taking a moment…') =>
  useCommandAnswerStore.getState().show(place, { command: 'deploy', text, pending: true });
const answerAt = (place: string) => useCommandAnswerStore.getState().answers[place];

describe('a command that answers later', () => {
  beforeEach(() => {
    useCommandAnswerStore.setState({ answers: {} });
  });

  it('replaces the pending line with the answer', () => {
    waiting('ch-1');
    applyCommandResponse(frame('ephemeral', 'deployed'));
    expect(answerAt('ch-1')).toEqual(
      expect.objectContaining({ command: 'deploy', text: 'deployed', pending: false }),
    );
  });

  it('shows a final error the same way', () => {
    waiting('ch-1');
    applyCommandResponse(frame('error', '/deploy could not be reached'));
    expect(answerAt('ch-1')?.text).toBe('/deploy could not be reached');
  });

  it('clears the pending line once the answer is posted in the channel', () => {
    waiting('ch-1', '/deploy is working on it…');
    applyCommandResponse(frame('in_channel', ''));
    expect(answerAt('ch-1')).toBeUndefined();
  });

  it('leaves another channel and an earlier answer alone', () => {
    waiting('ch-2', '/deploy is working on it…');
    applyCommandResponse(frame('in_channel', '', 'ch-1'));
    expect(answerAt('ch-2')?.pending).toBe(true);

    useCommandAnswerStore.getState().show('ch-1', { command: 'deploy', text: 'done', pending: false });
    applyCommandResponse(frame('in_channel', ''));
    expect(answerAt('ch-1')?.text).toBe('done');
  });

  it('keeps an answer for each place, so a second command does not replace the first', () => {
    waiting('ch-1');
    waiting('ch-2');
    applyCommandResponse(frame('ephemeral', 'deployed to staging', 'ch-2'));

    expect(answerAt('ch-1')?.pending).toBe(true);
    expect(answerAt('ch-2')?.text).toBe('deployed to staging');
  });

  it('brings an answer from a thread back to that thread, not to its channel', () => {
    const thread = answerPlace('ch-1', 'root-1');
    waiting('ch-1');
    waiting(thread);
    applyCommandResponse(frame('ephemeral', 'deployed', 'ch-1', 'root-1'));

    expect(thread).toBe('thread:root-1');
    expect(answerAt(thread)?.text).toBe('deployed');
    expect(answerAt('ch-1')?.pending).toBe(true);
  });

  it('stops waiting when the connection comes back, since an answer may have been lost', () => {
    waiting('ch-1');
    useCommandAnswerStore.getState().show('ch-2', { command: 'deploy', text: 'done', pending: false });

    giveUpOnAnswers();

    expect(answerAt('ch-1')).toEqual(
      expect.objectContaining({ text: 'No answer from /deploy', pending: false }),
    );
    expect(answerAt('ch-2')?.text).toBe('done');
  });
});
