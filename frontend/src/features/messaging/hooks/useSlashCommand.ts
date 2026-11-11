import { useCallback, useEffect } from 'react';
import { commandResultText, parseCommand, runCommand } from '@/lib/slashCommands';
import { toUserMessage } from '@/lib/errors';
import { ANSWER_WAIT_MS, answerPlace, useCommandAnswerStore } from '@/stores/commandAnswer';

const SHOWN_FOR_MS = 10_000;

export function useSlashCommand(
  channelId: string | undefined,
  threadParentId: string | undefined,
  instanceUrl: string | undefined,
) {
  const place = channelId ? answerPlace(channelId, threadParentId) : null;
  const answer = useCommandAnswerStore((s) => (place ? s.answers[place] : undefined));
  const show = useCommandAnswerStore((s) => s.show);
  const dismissAnswer = useCommandAnswerStore((s) => s.dismiss);
  const giveUp = useCommandAnswerStore((s) => s.giveUp);

  useEffect(() => {
    if (!answer || !place) return undefined;
    const timer = answer.pending
      ? setTimeout(() => giveUp(place), Math.max(0, answer.since + ANSWER_WAIT_MS - Date.now()))
      : setTimeout(() => dismissAnswer(place), SHOWN_FOR_MS);
    return () => clearTimeout(timer);
  }, [answer, place, giveUp, dismissAnswer]);

  const run = useCallback(
    async (content: string): Promise<boolean> => {
      const command = parseCommand(content);
      if (!command || !channelId || !place) return false;
      let result;
      try {
        result = await runCommand(channelId, content, instanceUrl, threadParentId);
      } catch (e) {
        show(place, { command: command.command, text: toUserMessage(e), pending: false });
        return true;
      }
      if (!result) {
        show(place, {
          command: command.command,
          text: `/${command.command} is not a command here. To post it as text, put it in \`backticks\`.`,
          pending: false,
        });
        throw new Error(`Unknown command /${command.command}`);
      }
      if (result.response_type === 'in_channel') dismissAnswer(place);
      else
        show(place, {
          command: command.command,
          text: commandResultText(result),
          pending: result.response_type === 'pending',
        });
      return true;
    },
    [channelId, threadParentId, place, instanceUrl, show, dismissAnswer],
  );

  const dismiss = useCallback(() => {
    if (place) dismissAnswer(place);
  }, [place, dismissAnswer]);

  return { run, answer: answer?.text ?? null, pending: answer?.pending ?? false, dismiss };
}
