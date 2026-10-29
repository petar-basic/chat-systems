import { getApiForInstance } from '@/shared/hooks/useCurrentApi';
import { logger } from '@/lib/logger';
import type { ActiveHuddle } from '@/stores/huddle';
import type { HuddleTransport, TransportHandlers } from './HuddleTransport';
import type { SfuTransport } from './SfuTransport';
import { MeshManager } from './MeshManager';

export const SFU_FLAG = 'chat_huddle_sfu';

const wantsSfu = (): boolean => {
  try {
    return localStorage.getItem(SFU_FLAG) === '1';
  } catch {
    return false;
  }
};

export async function openTransport(
  active: ActiveHuddle,
  iceServers: RTCIceServer[],
  send: (msg: Record<string, unknown>) => void,
  handlers: TransportHandlers,
): Promise<HuddleTransport> {
  if (wantsSfu()) {
    let sfu: SfuTransport | null = null;
    try {
      const room = await getApiForInstance(active.instanceUrl).typed((c) =>
        c.POST('/huddles/{huddle_id}/sfu-token', { params: { path: { huddle_id: active.huddleId } } }),
      );
      const { SfuTransport } = await import('./SfuTransport');
      sfu = new SfuTransport(room, iceServers, handlers);
      await sfu.join();
      return sfu;
    } catch (err) {
      sfu?.close();
      logger.error('openTransport', 'sfu', err);
    }
  }
  return new MeshManager(active.huddleId, active.selfUserId, iceServers, send, handlers);
}
