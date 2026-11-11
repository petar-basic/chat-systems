import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import i18n from '@/lib/i18n';
import { render, screen, fireEvent } from '@testing-library/react';
import { ChannelIntro } from './ChannelIntro';
import type { Channel } from '@/stores/workspace';

const channel = (over: Partial<Channel> = {}) =>
  ({
    id: 'ch-1',
    name: 'design',
    channel_type: 'public',
    is_default: false,
    description: null,
    topic: null,
    ...over,
  }) as Channel;

describe('ChannelIntro', () => {
  it('says what the channel is for', () => {
    render(<ChannelIntro channel={channel({ description: 'Mockups and reviews' })} />);
    expect(screen.getByText('This is the very beginning of #design')).toBeInTheDocument();
    expect(screen.getByTestId('channel-intro-purpose')).toHaveTextContent('Mockups and reviews');
  });

  it('explains #general when nobody described it', () => {
    render(<ChannelIntro channel={channel({ name: 'general', is_default: true })} />);
    expect(screen.getByTestId('channel-intro-purpose')).toHaveTextContent(
      'Everyone in the workspace is here',
    );
  });

  it('offers only what the viewer may do', () => {
    const onAddPeople = vi.fn();
    const onAddBookmark = vi.fn();
    const { rerender } = render(
      <ChannelIntro channel={channel()} onAddPeople={onAddPeople} onAddBookmark={onAddBookmark} />,
    );
    fireEvent.click(screen.getByTestId('channel-intro-add-people'));
    fireEvent.click(screen.getByTestId('channel-intro-add-bookmark'));
    expect(onAddPeople).toHaveBeenCalled();
    expect(onAddBookmark).toHaveBeenCalled();

    rerender(<ChannelIntro channel={channel()} />);
    expect(screen.queryByTestId('channel-intro-add-people')).toBeNull();
    expect(screen.queryByTestId('channel-intro-add-bookmark')).toBeNull();
  });
});

describe('ChannelIntro in Japanese', () => {
  const hostile = '<img src=x onerror="alert(1)"> {{name}}';
  beforeEach(async () => {
    await i18n.changeLanguage('ja');
  });
  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('names the channel as text and says who can describe it', () => {
    render(
      <ChannelIntro channel={channel({ name: hostile })} onAddPeople={vi.fn()} onAddBookmark={vi.fn()} />,
    );
    expect(screen.getByRole('heading', { name: `#${hostile} の始まりです` })).toBeInTheDocument();
    expect(screen.getByTestId('channel-intro-purpose').textContent).toBe(
      'このチャンネルの目的はまだ書かれていません。ここに投稿できる人なら誰でも、チャンネル詳細で説明を追加できます。',
    );
    expect(screen.getByTestId('channel-intro-add-people')).toHaveTextContent('メンバーを追加');
    expect(screen.getByTestId('channel-intro-add-bookmark')).toHaveTextContent('ブックマークを追加');
    expect(document.querySelector('[onerror]')).toBeNull();
  });
});
