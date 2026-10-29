import { getApiForInstance } from '@/shared/hooks/useCurrentApi';
import type { ActiveHuddle } from '@/stores/huddle';
import type { HuddleTransport, TransportHandlers } from './HuddleTransport';

export async function openSfu(
  active: ActiveHuddle,
  iceServers: RTCIceServer[],
  handlers: TransportHandlers,
): Promise<HuddleTransport> {
  const room = await getApiForInstance(active.instanceUrl).typed((c) =>
    c.POST('/huddles/{huddle_id}/sfu-token', { params: { path: { huddle_id: active.huddleId } } }),
  );
  const { SfuTransport } = await import('./SfuTransport');
  const sfu = new SfuTransport(room, iceServers, handlers);
  try {
    await sfu.join();
  } catch (err) {
    sfu.close();
    throw err;
  }
  return sfu;
}
