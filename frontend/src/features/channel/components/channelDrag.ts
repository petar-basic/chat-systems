import type { DragEvent } from 'react';

export const CHANNEL_DRAG_TYPE = 'application/x-chat-channel';

export function acceptsChannelDrop(onDropChannel: (channelId: string) => void) {
  return {
    onDragOver: (event: DragEvent) => {
      if (event.dataTransfer.types.includes(CHANNEL_DRAG_TYPE)) event.preventDefault();
    },
    onDrop: (event: DragEvent) => {
      const channelId = event.dataTransfer.getData(CHANNEL_DRAG_TYPE);
      if (!channelId) return;
      event.preventDefault();
      onDropChannel(channelId);
    },
  };
}
