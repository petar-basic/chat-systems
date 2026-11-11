import type { EventOfType } from './serverEvents';
import { answerPlace, useCommandAnswerStore } from '@/stores/commandAnswer';

export function applyCommandResponse(event: EventOfType<'command.response'>) {
  const answers = useCommandAnswerStore.getState();
  const place = answerPlace(event.channel_id, event.thread_parent_id);
  if (event.response_type === 'in_channel') {
    answers.settle(place);
    return;
  }
  answers.show(place, { command: event.command, text: event.text, pending: false });
}

export function giveUpOnAnswers() {
  useCommandAnswerStore.getState().giveUp();
}
