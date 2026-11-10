import { describe, it, expect, vi, afterEach } from 'vitest';
import { act, renderHook } from '@testing-library/react';
import i18n from '@/lib/i18n';
import type { Message } from '@/stores/workspace';
import { useMessageActions } from './useMessageActions';

const save = vi.fn();
vi.mock('@/features/later', () => ({ useLaterActions: () => ({ save: { mutate: save } }) }));

const message = {
  id: 'm-1',
  channel_id: 'gone',
  user_id: 'ghost',
  content: 'hi',
  metadata: {},
} as unknown as Message;

describe('useMessageActions in Japanese', () => {
  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('names an unknown author and a channel you left in Japanese', async () => {
    await i18n.changeLanguage('ja');
    const { result } = renderHook(() =>
      useMessageActions({
        workspaceId: 'ws-1',
        currentWsInstanceUrl: undefined,
        channels: [],
        conversations: [],
        userId: 'me',
        getUser: () => undefined,
      }),
    );
    act(() => result.current.handleForwardMessage(message));
    expect(result.current.forwarding).toMatchObject({
      authorName: '不明なユーザー',
      origin: '退出したチャンネル',
    });
  });
});
