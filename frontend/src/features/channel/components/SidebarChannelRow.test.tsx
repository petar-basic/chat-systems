import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import i18n from '@/lib/i18n';
import { SidebarChannelRow } from './SidebarChannelRow';
import type { Channel } from '@/stores/workspace';

const channel = { id: 'ch-1', name: 'design', channel_type: 'public' } as Channel;

describe('SidebarChannelRow', () => {
  it('moves a channel into a section from its menu', () => {
    const onMove = vi.fn();
    render(
      <SidebarChannelRow
        channel={channel}
        active={false}
        muted={false}
        unread={false}
        mentioned={false}
        unreadCount={undefined}
        mentionCount={undefined}
        onSelect={vi.fn()}
        moveTargets={[
          { id: null, name: 'Channels' },
          { id: 'sec-1', name: 'Projects' },
        ]}
        sectionId={null}
        onMove={onMove}
      />,
    );
    fireEvent.click(screen.getByTestId('sidebar-channel-move'));
    expect(screen.queryByText('Channels')).not.toBeInTheDocument();
    fireEvent.click(screen.getByText('Projects'));
    expect(onMove).toHaveBeenCalledWith('ch-1', 'sec-1');
  });
});

describe('SidebarChannelRow in Japanese', () => {
  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('names the move button after the channel, as text', async () => {
    const name = '<img src=x onerror="alert(1)"> {{name}}';
    await i18n.changeLanguage('ja');
    render(
      <SidebarChannelRow
        channel={{ ...channel, name }}
        active={false}
        muted
        unread={false}
        mentioned={false}
        unreadCount={undefined}
        mentionCount={undefined}
        onSelect={vi.fn()}
        moveTargets={[
          { id: null, name: 'チャンネル' },
          { id: 'sec-1', name: 'Projects' },
        ]}
        sectionId={'sec-1'}
        onMove={vi.fn()}
      />,
    );
    const move = screen.getByRole('button', { name: `${name} をセクションに移動` });
    expect(move).toHaveAttribute('data-tooltip', '移動…');
    expect(screen.getByLabelText('ミュート中')).toBeInTheDocument();
    fireEvent.click(move);
    expect(screen.getByText('移動先')).toBeInTheDocument();
    expect(document.querySelector('img')).toBeNull();
  });
});
