import i18n from '@/lib/i18n';
import type { InstalledApp } from '@/features/workspace/hooks/useAppDirectory';

export function channelText(app: InstalledApp) {
  if (app.every_channel) return i18n.t('workspace:settings.apps.everyChannel');
  const names = app.channels.map((channel) => `#${channel.name}`);
  if (app.hidden_channels > 0) {
    names.push(i18n.t('workspace:settings.apps.hiddenChannels', { count: app.hidden_channels }));
  }
  return names.length === 0
    ? i18n.t('workspace:settings.apps.noChannels')
    : names.join(i18n.t('common:listSeparator'));
}
