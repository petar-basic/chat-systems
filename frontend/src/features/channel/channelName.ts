export const CHANNEL_NAME_MAX = 80;

export function channelNameFrom(typed: string) {
  return typed.toLowerCase().replace(/\s/g, '-');
}
