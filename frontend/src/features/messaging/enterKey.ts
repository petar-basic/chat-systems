import type { EnterKey } from '@/stores/display';

export type EnterOutcome = 'send' | 'newline' | 'default';

interface EnterPress {
  mod: boolean;
  picking: boolean;
  inBlock: boolean;
}

export function enterOutcome(mode: EnterKey, { mod, picking, inBlock }: EnterPress): EnterOutcome {
  if (picking) return 'default';
  if (mod) return 'send';
  if (inBlock) return 'default';
  return mode === 'send' ? 'send' : 'newline';
}
