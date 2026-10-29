import type { EventOfType } from './serverEvents';
import { useCommandAnswerStore } from '@/stores/commandAnswer';

export function applyCommandResponse(event: EventOfType<'command.response'>) {
  const answers = useCommandAnswerStore.getState();
  if (event.response_type === 'in_channel') {
    answers.settle(event.channel_id);
    return;
  }
  answers.show({ channelId: event.channel_id, text: event.text, pending: false });
}
