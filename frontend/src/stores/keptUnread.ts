import { create } from 'zustand';
import { useWorkspaceStore } from './workspace';

interface KeptUnreadState {
  channelId: string | null;
}

export const useKeptUnread = create<KeptUnreadState>(() => ({ channelId: null }));

export function keepUnread(channelId: string) {
  const { currentChannel, currentConversationId } = useWorkspaceStore.getState();
  if (currentChannel?.id === channelId || currentConversationId === channelId) {
    useKeptUnread.setState({ channelId });
  }
}

export function releaseUnread(channelId: string) {
  if (useKeptUnread.getState().channelId === channelId) useKeptUnread.setState({ channelId: null });
}

export function isKeptUnread(channelId: string): boolean {
  return useKeptUnread.getState().channelId === channelId;
}
