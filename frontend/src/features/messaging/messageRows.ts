import type { VirtualRow } from './VirtualMessageList';
import { isGrouped, isNewDay, type GroupableMessage } from './messageGrouping';

type Groupable = GroupableMessage & {
  id: string;
  content: string;
};

export type MessageRow<M extends Groupable> =
  | (VirtualRow & { kind: 'day'; at: string })
  | (VirtualRow & { kind: 'message'; message: M; grouped: boolean });

const DAY_ROW_HEIGHT = 36;
const HEADER_HEIGHT = 22;
const LINE_HEIGHT = 21;
const CHARS_PER_LINE = 90;

/**
 * A first guess at a row's height so the scrollbar is roughly right before the
 * virtualizer has measured anything. It only has to be in the right order of
 * magnitude — every mounted row is measured for real.
 */
function estimateHeight(content: string, grouped: boolean): number {
  const lines = content.split('\n').reduce((total, line) => {
    return total + Math.max(1, Math.ceil(line.length / CHARS_PER_LINE));
  }, 0);
  return (grouped ? 0 : HEADER_HEIGHT) + lines * LINE_HEIGHT + 6;
}

/**
 * Grouping depends on the previous message, so it cannot be decided per rendered
 * item once the list is windowed — the previous message is often not mounted.
 * The whole list is flattened here instead, and the virtualizer only ever sees
 * rows that already know what they are.
 */
export function buildMessageRows<M extends Groupable>(messages: M[]): Array<MessageRow<M>> {
  const rows: Array<MessageRow<M>> = [];

  messages.forEach((message, i) => {
    const previous = messages[i - 1];
    const newDay = isNewDay(previous, message);
    if (newDay) {
      rows.push({
        kind: 'day',
        key: `day-${message.id}`,
        at: message.created_at,
        estimatedHeight: DAY_ROW_HEIGHT,
      });
    }
    const grouped = !newDay && isGrouped(previous, message);
    rows.push({
      kind: 'message',
      key: message.id,
      message,
      grouped,
      estimatedHeight: estimateHeight(message.content, grouped),
    });
  });

  return rows;
}
