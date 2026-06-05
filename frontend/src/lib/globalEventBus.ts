import type { AppServerEvent, ServerEventType, EventOfType } from './serverEvents';

export type ServerEvent = AppServerEvent;

type AnyHandler = (event: AppServerEvent) => void;

class GlobalEventBus {
  private handlers: Map<string, Set<AnyHandler>> = new Map();

  on<T extends ServerEventType>(type: T, handler: (event: EventOfType<T>) => void): () => void {
    if (!this.handlers.has(type)) {
      this.handlers.set(type, new Set());
    }
    this.handlers.get(type)!.add(handler as AnyHandler);
    return () => this.handlers.get(type)?.delete(handler as AnyHandler);
  }

  emit(event: AppServerEvent) {
    const handlers = this.handlers.get(event.type);
    if (handlers) {
      handlers.forEach((h) => h(event));
    }
  }
}

export const globalEventBus = new GlobalEventBus();
