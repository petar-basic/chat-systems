import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import type { components } from '@/api/schema';
import type { Message } from '@/stores/workspace';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';
import { ErrorLabels, QUERY_KEYS } from '@/shared/constants';
import { toast } from '@/shared/components/Toast';
import { toUserMessage } from '@/lib/errors';
import { patchMessageEverywhere, placeOwnMessage } from '@/lib/messagePatch';

export type PollSummary = components['schemas']['PollSummary'];
export type CreatePollRequest = components['schemas']['CreatePollRequest'];
type Located = Pick<Message, 'id' | 'channel_id' | 'thread_parent_id'>;

export function tallyOf(poll: PollSummary): string {
  return `${poll.voters}:${poll.options.map((option) => option.votes).join(',')}:${poll.closed_at ?? ''}`;
}

export function withVote(poll: PollSummary, before: string[], after: string[]): PollSummary {
  const was = new Set(before);
  const now = new Set(after);
  const voted = before.length > 0;
  const votes = after.length > 0;
  return {
    ...poll,
    voters: poll.voters + (votes && !voted ? 1 : 0) - (voted && !votes ? 1 : 0),
    options: poll.options.map((option) => ({
      ...option,
      votes: option.votes + (now.has(option.id) ? 1 : 0) - (was.has(option.id) ? 1 : 0),
    })),
  };
}

function setPoll(queryClient: ReturnType<typeof useQueryClient>, message: Located, poll: PollSummary) {
  patchMessageEverywhere(queryClient, message, (m) => ({
    ...m,
    metadata: { ...m.metadata, poll },
  }));
}

export function useMyVote(pollId: string, instanceUrl?: string) {
  return useQuery({
    queryKey: QUERY_KEYS.pollVote(pollId),
    queryFn: async () => {
      const res = await getApiForInstance(instanceUrl).typed((c) =>
        c.GET('/polls/{poll_id}/vote', { params: { path: { poll_id: pollId } } }),
      );
      return res.option_ids;
    },
    staleTime: Infinity,
  });
}

export function usePollVoters(poll: PollSummary, instanceUrl?: string) {
  return useQuery({
    queryKey: QUERY_KEYS.pollVoters(poll.id, tallyOf(poll)),
    queryFn: async () => {
      const res = await getApiForInstance(instanceUrl).typed((c) =>
        c.GET('/polls/{poll_id}/votes', { params: { path: { poll_id: poll.id } } }),
      );
      return res.data;
    },
    enabled: !poll.anonymous && poll.voters > 0,
    placeholderData: (previous) => previous,
    staleTime: Infinity,
  });
}

export function useVote(message: Located, poll: PollSummary, instanceUrl?: string) {
  const queryClient = useQueryClient();
  const key = QUERY_KEYS.pollVote(poll.id);
  return useMutation({
    mutationFn: (optionIds: string[]) =>
      getApiForInstance(instanceUrl).typed((c) =>
        c.PUT('/polls/{poll_id}/vote', {
          params: { path: { poll_id: poll.id } },
          body: { option_ids: optionIds },
        }),
      ),
    onMutate: async (optionIds) => {
      await queryClient.cancelQueries({ queryKey: key });
      const mine = queryClient.getQueryData<string[]>(key) ?? [];
      queryClient.setQueryData(key, optionIds);
      setPoll(queryClient, message, withVote(poll, mine, optionIds));
      return { mine, poll };
    },
    onError: (err, _optionIds, context) => {
      if (context) {
        queryClient.setQueryData(key, context.mine);
        setPoll(queryClient, message, context.poll);
      }
      toast.error(toUserMessage(err, ErrorLabels.VoteFailed));
    },
    onSuccess: (result) => {
      queryClient.setQueryData(key, result.option_ids);
      setPoll(queryClient, message, result.poll);
    },
  });
}

export function useClosePoll(message: Located, pollId: string, instanceUrl?: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: () =>
      getApiForInstance(instanceUrl).typed((c) =>
        c.POST('/polls/{poll_id}/close', { params: { path: { poll_id: pollId } } }),
      ),
    onSuccess: (poll) => setPoll(queryClient, message, poll),
    onError: (err) => toast.error(toUserMessage(err, ErrorLabels.PollCloseFailed)),
  });
}

export function useCreatePoll(channelId: string, instanceUrl?: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (body: CreatePollRequest) =>
      getApiForInstance(instanceUrl).typed((c) =>
        c.POST('/channels/{ch_id}/polls', { params: { path: { ch_id: channelId } }, body }),
      ),
    onSuccess: (message) => placeOwnMessage(queryClient, message as Message),
  });
}
