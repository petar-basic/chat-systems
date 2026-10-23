import { describe, it, expect, vi, beforeEach } from 'vitest';
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { createRef } from 'react';
import { QueryClient, QueryClientProvider, useQuery, type InfiniteData } from '@tanstack/react-query';
import type { Message } from '@/stores/workspace';
import { QUERY_KEYS } from '@/shared/constants';
import { PollCard, type PollKeyVote } from './PollCard';
import type { PollSummary } from './usePolls';

const calls = { get: vi.fn(), put: vi.fn(), post: vi.fn() };
vi.mock('@/shared/hooks/useCurrentApi', async () => {
  const { typedApiMock } = await import('@/test/typedApiMock');
  return {
    getApiForInstance: () =>
      typedApiMock({
        get: (...args) => calls.get(...args),
        put: (...args) => calls.put(...args),
        post: (...args) => calls.post(...args),
      }),
  };
});
vi.mock('@/shared/components/Toast', () => ({ toast: { error: vi.fn(), success: vi.fn() } }));

const SELF = 'u-self';

function pollOf(overrides: Partial<PollSummary> = {}): PollSummary {
  return {
    id: 'p-1',
    question: 'Lunch?',
    multiple_choice: false,
    anonymous: false,
    created_by: 'u-other',
    closes_at: null,
    closed_at: null,
    voters: 2,
    options: [
      { id: 'o-thu', label: 'Thursday', votes: 2 },
      { id: 'o-fri', label: 'Friday', votes: 0 },
    ],
    ...overrides,
  };
}

function messageWith(poll: PollSummary): Message {
  return {
    id: 'm-1',
    channel_id: 'c-1',
    user_id: 'u-other',
    content: poll.question,
    created_at: '2026-10-23T10:00:00Z',
    updated_at: '2026-10-23T10:00:00Z',
    reply_count: 0,
    is_pinned: false,
    metadata: { poll },
  } as Message;
}

function Harness({ keyVoteRef }: { keyVoteRef?: React.RefObject<PollKeyVote | null> }) {
  const { data } = useQuery<InfiniteData<{ data: Message[] }>>({
    queryKey: QUERY_KEYS.messages('c-1'),
    queryFn: () => new Promise(() => {}),
    staleTime: Infinity,
  });
  const message = data?.pages[0].data[0];
  const poll = message?.metadata?.poll;
  if (!message || !poll) return null;
  return <PollCard message={message} poll={poll} currentUserId={SELF} keyVoteRef={keyVoteRef} />;
}

function renderCard(
  poll: PollSummary,
  mine: string[] = [],
  keyVoteRef?: React.RefObject<PollKeyVote | null>,
) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  client.setQueryData(QUERY_KEYS.messages('c-1'), {
    pages: [{ data: [messageWith(poll)] }],
    pageParams: [null],
  });
  calls.get.mockImplementation(async (path: string) =>
    path.endsWith('/votes')
      ? {
          data: [
            { option_id: 'o-thu', user_ids: ['u-a', 'u-b'] },
            { option_id: 'o-fri', user_ids: [] },
          ],
        }
      : { option_ids: mine },
  );
  render(
    <QueryClientProvider client={client}>
      <Harness keyVoteRef={keyVoteRef} />
    </QueryClientProvider>,
  );
  return client;
}

const options = () => screen.getAllByTestId('poll-option');
const countsShown = () => screen.getAllByTestId('poll-option-count').map((el) => el.textContent);

describe('PollCard', () => {
  beforeEach(() => {
    calls.get.mockReset();
    calls.put.mockReset();
    calls.post.mockReset();
  });

  it('shows the question, a row per option with its count, and the caller own choice', async () => {
    renderCard(pollOf(), ['o-thu']);
    expect(screen.getByTestId('poll-question')).toHaveTextContent('Lunch?');
    expect(countsShown()).toEqual(['2', '0']);
    await waitFor(() => expect(options()[0]).toHaveAttribute('aria-pressed', 'true'));
    expect(options()[1]).toHaveAttribute('aria-pressed', 'false');
    expect(screen.getByTestId('poll-footer')).toHaveTextContent('2 votes');
    expect(await screen.findAllByTestId('poll-option-voters')).toHaveLength(1);
  });

  it('moves the vote at once and keeps what the server answers', async () => {
    let answer: (value: unknown) => void = () => {};
    calls.put.mockReturnValue(new Promise((resolve) => (answer = resolve)));
    renderCard(pollOf(), ['o-thu']);
    await waitFor(() => expect(options()[0]).toHaveAttribute('aria-pressed', 'true'));

    fireEvent.click(options()[1]);
    await waitFor(() => expect(options()[1]).toHaveAttribute('aria-pressed', 'true'));
    expect(options()[0]).toHaveAttribute('aria-pressed', 'false');
    expect(countsShown()).toEqual(['1', '1']);
    expect(calls.put).toHaveBeenCalledWith('/polls/{poll_id}/vote', { option_ids: ['o-fri'] });

    await act(async () =>
      answer({
        option_ids: ['o-fri'],
        poll: pollOf({
          voters: 3,
          options: [
            { id: 'o-thu', label: 'Thursday', votes: 1 },
            { id: 'o-fri', label: 'Friday', votes: 2 },
          ],
        }),
      }),
    );
    expect(countsShown()).toEqual(['1', '2']);
  });

  it('puts everything back when the vote fails', async () => {
    calls.put.mockRejectedValue(new Error('closed'));
    renderCard(pollOf(), []);
    await waitFor(() => expect(calls.get).toHaveBeenCalled());

    fireEvent.click(options()[1]);
    await waitFor(() => expect(calls.put).toHaveBeenCalled());
    await waitFor(() => expect(countsShown()).toEqual(['2', '0']));
    expect(options()[1]).toHaveAttribute('aria-pressed', 'false');
  });

  it('withdraws a vote by pressing the chosen row again', async () => {
    calls.put.mockResolvedValue({
      option_ids: [],
      poll: pollOf({
        voters: 1,
        options: [
          { id: 'o-thu', label: 'Thursday', votes: 1 },
          { id: 'o-fri', label: 'Friday', votes: 0 },
        ],
      }),
    });
    renderCard(pollOf(), ['o-thu']);
    await waitFor(() => expect(options()[0]).toHaveAttribute('aria-pressed', 'true'));
    fireEvent.click(options()[0]);
    await waitFor(() => expect(calls.put).toHaveBeenCalledWith('/polls/{poll_id}/vote', { option_ids: [] }));
  });

  it('adds to the answer on a multiple choice poll', async () => {
    calls.put.mockReturnValue(new Promise(() => {}));
    renderCard(pollOf({ multiple_choice: true }), ['o-thu']);
    await waitFor(() => expect(options()[0]).toHaveAttribute('aria-pressed', 'true'));
    fireEvent.click(options()[1]);
    await waitFor(() =>
      expect(calls.put).toHaveBeenCalledWith('/polls/{poll_id}/vote', { option_ids: ['o-thu', 'o-fri'] }),
    );
    await waitFor(() => expect(countsShown()).toEqual(['2', '1']));
  });

  it('votes from the keyboard by option number', async () => {
    calls.put.mockReturnValue(new Promise(() => {}));
    const keyVoteRef = createRef<PollKeyVote | null>();
    renderCard(pollOf(), [], keyVoteRef);
    await waitFor(() => expect(calls.get).toHaveBeenCalled());
    act(() => keyVoteRef.current?.(1));
    await waitFor(() =>
      expect(calls.put).toHaveBeenCalledWith('/polls/{poll_id}/vote', { option_ids: ['o-fri'] }),
    );
  });

  it('never asks who voted on an anonymous poll, and a closed poll takes no votes', async () => {
    renderCard(pollOf({ anonymous: true, closed_at: '2026-10-23T12:00:00Z' }), []);
    await waitFor(() => expect(calls.get).toHaveBeenCalledTimes(1));
    expect(calls.get).not.toHaveBeenCalledWith('/polls/{poll_id}/votes');
    expect(screen.getByTestId('poll-footer')).toHaveTextContent('Closed');
    expect(options()[0]).toBeDisabled();
    expect(screen.queryByRole('button', { name: 'Close poll' })).toBeNull();
  });

  it('lets its creator close it', async () => {
    calls.post.mockResolvedValue(pollOf({ created_by: SELF, closed_at: '2026-10-23T12:00:00Z' }));
    renderCard(pollOf({ created_by: SELF }), []);
    fireEvent.click(screen.getByRole('button', { name: 'Close poll' }));
    await waitFor(() => expect(screen.getByTestId('poll-footer')).toHaveTextContent('Closed'));
    expect(calls.post).toHaveBeenCalledWith('/polls/{poll_id}/close', undefined);
  });
});
