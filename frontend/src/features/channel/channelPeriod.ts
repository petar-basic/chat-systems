import type { ChannelPeriod, ChannelRetention } from '@/hooks/queries/useRetention';

export interface PeriodChoice {
  mode: 'workspace' | 'forever' | 'days';
  days: string;
}

export function choiceOf(retention: ChannelRetention): PeriodChoice {
  const own = retention.channel;
  if (!own) return { mode: 'workspace', days: '30' };
  return own.message_days
    ? { mode: 'days', days: String(own.message_days) }
    : { mode: 'forever', days: '30' };
}

export function periodOf(choice: PeriodChoice): ChannelPeriod | 'invalid' {
  if (choice.mode === 'workspace') return 'workspace';
  if (choice.mode === 'forever') return null;
  const days = Number(choice.days);
  return Number.isInteger(days) && days >= 1 ? days : 'invalid';
}
