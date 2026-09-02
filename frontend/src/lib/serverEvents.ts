import type { ServerFrame } from '@/api/serverFrames';

export type PresenceValue = 'online' | 'away' | 'offline';

export type AppServerEvent = ServerFrame & { stream_id?: string };

export type ServerEventType = AppServerEvent['type'];
export type EventOfType<T extends ServerEventType> = Extract<AppServerEvent, { type: T }>;
