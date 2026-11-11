import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import i18n from '@/lib/i18n';
import { render, screen } from '@testing-library/react';
import { ChannelPreviewBar } from './ChannelPreviewBar';
import type { Channel } from '@/stores/workspace';

vi.mock('./hooks/useChannels', () => ({
  useJoinChannel: () => ({ mutate: vi.fn(), isPending: false }),
}));

const hostile = '<img src=x onerror="alert(1)"> {{name}}';

describe('ChannelPreviewBar in Japanese', () => {
  beforeEach(async () => {
    await i18n.changeLanguage('ja');
  });
  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('names the channel being viewed as text and offers to join', () => {
    render(<ChannelPreviewBar channel={{ id: 'c1', workspace_id: 'w1', name: hostile } as Channel} />);
    expect(screen.getByTestId('channel-preview').textContent).toBe(
      `#${hostile} を閲覧中です。投稿するには、チャンネルに参加してください。チャンネルに参加`,
    );
    expect(screen.getByText(`#${hostile}`)).toHaveClass('font-semibold');
    expect(document.querySelector('[onerror]')).toBeNull();
  });
});
