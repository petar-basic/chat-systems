import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import i18n from '@/lib/i18n';
import { render, screen } from '@testing-library/react';
import PinnedMessagesPanel from './PinnedMessagesPanel';

vi.mock('./hooks/useChannels', () => ({
  useChannelPins: () => ({ data: [], isLoading: false }),
}));

describe('PinnedMessagesPanel in Japanese', () => {
  beforeEach(async () => {
    await i18n.changeLanguage('ja');
  });
  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('says nothing is pinned yet', () => {
    render(<PinnedMessagesPanel channelId="c1" onClose={vi.fn()} />);
    expect(screen.getByText('ピン留めしたメッセージ')).toBeInTheDocument();
    expect(screen.getByText('ピン留めしたメッセージはまだありません')).toBeInTheDocument();
    expect(
      screen.getByText('大事なメッセージをピン留めしておくと、あとで見つけやすくなります'),
    ).toBeInTheDocument();
  });
});
