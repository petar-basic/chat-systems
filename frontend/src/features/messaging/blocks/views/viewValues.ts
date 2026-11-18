import type { components } from '@/api/schema';
import type { AppView, ViewValues } from '@/stores/appViews';

type Block = components['schemas']['Block'];
type BlockElement = components['schemas']['BlockElement'];

export type InputBlock = Extract<Block, { type: 'input' }>;

const present = (value: string | null | undefined): string[] => (value ? [value] : []);

export function initialValue(element: BlockElement): string[] {
  switch (element.type) {
    case 'plain_text_input':
      return present(element.initial_value);
    case 'checkboxes':
      return (element.initial_options ?? []).flatMap((option) => present(option.value));
    case 'radio_buttons':
    case 'static_select':
      return present(element.initial_option?.value);
    case 'users_select':
      return present(element.initial_user);
    case 'conversations_select':
      return present(element.initial_conversation);
    case 'datepicker':
      return present(element.initial_date);
    default:
      return [];
  }
}

export function inputsOf(view: AppView): InputBlock[] {
  return view.blocks.filter(
    (block): block is InputBlock =>
      block.type === 'input' && !!block.block_id && 'action_id' in block.element && !!block.element.action_id,
  );
}

export function valueOf(block: InputBlock, stored: ViewValues | undefined): string[] {
  const actionId = 'action_id' in block.element ? block.element.action_id : null;
  if (!block.block_id || !actionId) return [];
  return stored?.[block.block_id]?.[actionId] ?? initialValue(block.element);
}

export function currentValues(view: AppView, stored: ViewValues | undefined): ViewValues {
  const values: ViewValues = {};
  for (const block of inputsOf(view)) {
    const actionId = 'action_id' in block.element ? block.element.action_id : null;
    if (!block.block_id || !actionId) continue;
    values[block.block_id] = { [actionId]: valueOf(block, stored) };
  }
  return values;
}
