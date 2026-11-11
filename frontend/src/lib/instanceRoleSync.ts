import { globalEventBus } from './globalEventBus';
import { logger } from './logger';
import { useInstanceStore } from '@/stores/instances';

export function followInstanceRole(): () => void {
  return globalEventBus.on('user.instance_role_changed', (_event, instanceUrl) => {
    if (!instanceUrl) return;
    useInstanceStore
      .getState()
      .refreshInstanceUser(instanceUrl)
      .catch((err: unknown) => logger.error('instanceRoleSync', 'user.instance_role_changed', err));
  });
}
