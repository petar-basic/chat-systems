import type { InstalledApp } from '@/features/workspace/hooks/useAppDirectory';

export function channelText(app: InstalledApp) {
  if (app.every_channel) return 'Every channel';
  const names = app.channels.map((channel) => `#${channel.name}`);
  if (app.hidden_channels > 0) {
    names.push(
      `${app.hidden_channels} private ${app.hidden_channels === 1 ? 'channel' : 'channels'} you are not in`,
    );
  }
  return names.length === 0 ? 'No channels' : names.join(', ');
}
