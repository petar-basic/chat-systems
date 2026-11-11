import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import i18n from '@/lib/i18n';
import { useUserCache } from '@/stores/users';
import { VideoTile } from './VideoTile';

const hostile = '<img src=x onerror="alert(1)"> {{name}}';

const tile = (over: Partial<Parameters<typeof VideoTile>[0]> = {}) => (
  <VideoTile
    tileId="t-1"
    userId="u-x"
    kind="camera"
    stream={null}
    audioStream={null}
    muted={false}
    cameraOn={false}
    sharing={false}
    handRaised={false}
    isSelf={false}
    speaking={false}
    pinned={false}
    speakerId={null}
    {...over}
  />
);

describe('VideoTile in Japanese', () => {
  beforeEach(async () => {
    useUserCache.setState({ users: new Map([['u-x', { id: 'u-x', display_name: hostile } as never]]) });
    await i18n.changeLanguage('ja');
  });
  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it("names someone's shared screen as text", () => {
    render(tile({ kind: 'screen', tileId: 't-2' }));
    expect(screen.getByText(`${hostile} さんの画面`)).toBeInTheDocument();
    expect(screen.getByText('画面を待っています…')).toBeInTheDocument();
    expect(document.querySelector('[onerror]')).toBeNull();
  });

  it('marks yourself, sharing, a raised hand and pinning', () => {
    const { rerender } = render(tile({ isSelf: true, sharing: true, handRaised: true }));
    expect(screen.getByText(`${hostile}（自分）`)).toBeInTheDocument();
    expect(screen.getByText('共有中')).toBeInTheDocument();
    expect(screen.getByLabelText('挙手中')).toBeInTheDocument();

    rerender(tile());
    expect(screen.getByRole('button', { name: '固定表示' })).toBeInTheDocument();
    rerender(tile({ pinned: true }));
    expect(screen.getByRole('button', { name: '固定を解除' })).toBeInTheDocument();
  });
});
