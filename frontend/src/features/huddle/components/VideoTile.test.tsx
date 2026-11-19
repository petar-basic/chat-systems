import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import i18n from '@/lib/i18n';
import { useUserCache } from '@/stores/users';
import { useHuddleStore } from '@/stores/huddle';
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
    drawing={false}
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

describe('VideoTile drawing pen', () => {
  beforeEach(() => {
    useUserCache.setState({ users: new Map([['u-x', { id: 'u-x', display_name: 'Ana' } as never]]) });
    useHuddleStore.setState({
      penOn: null,
      participants: {
        'u-x': {
          userId: 'u-x',
          audioStream: null,
          cameraStream: null,
          screenStream: null,
          audioMuted: false,
          cameraOn: false,
          sharing: true,
          drawingEnabled: true,
          handRaised: false,
        },
      },
    });
  });

  it("picks up the pen on someone's screen and puts it down with Esc", () => {
    render(tile({ kind: 'screen', tileId: 'u-x:screen', sharing: true, drawing: true }));
    const pen = screen.getByRole('button', { name: 'Draw on this screen' });
    fireEvent.click(pen);
    expect(useHuddleStore.getState().penOn).toBe('u-x');
    expect(screen.getByRole('button', { name: 'Stop drawing (Esc)' })).toHaveAttribute(
      'aria-pressed',
      'true',
    );

    fireEvent.keyDown(window, { key: 'Escape' });
    expect(useHuddleStore.getState().penOn).toBeNull();
    expect(screen.getByRole('button', { name: 'Draw on this screen' })).toHaveAttribute(
      'aria-pressed',
      'false',
    );

    fireEvent.click(screen.getByRole('button', { name: 'Draw on this screen' }));
    fireEvent.click(screen.getByRole('button', { name: 'Stop drawing (Esc)' }));
    expect(useHuddleStore.getState().penOn).toBeNull();
  });

  it('offers no pen when the sharer turned drawing off, on your own screen, or on a face', () => {
    const { rerender } = render(tile({ kind: 'screen', sharing: true, drawing: false }));
    expect(screen.queryByTestId('huddle-pen')).toBeNull();
    rerender(tile({ kind: 'screen', sharing: true, drawing: true, isSelf: true }));
    expect(screen.queryByTestId('huddle-pen')).toBeNull();
    rerender(tile({ kind: 'camera', sharing: true, drawing: true }));
    expect(screen.queryByTestId('huddle-pen')).toBeNull();
  });

  it('drops the pen when the sharer turns drawing off or stops sharing', () => {
    const store = useHuddleStore.getState();
    store.setPen('u-x');
    store.setParticipantDrawing('u-x', false);
    expect(useHuddleStore.getState().penOn).toBeNull();

    store.setPen('u-x');
    store.setParticipantSharing('u-x', true);
    expect(useHuddleStore.getState().participants['u-x'].drawingEnabled).toBe(false);
    expect(useHuddleStore.getState().penOn).toBe('u-x');
    store.setParticipantSharing('u-x', false);
    expect(useHuddleStore.getState().penOn).toBeNull();
    store.setParticipantSharing('u-x', true);
    expect(useHuddleStore.getState().participants['u-x'].drawingEnabled).toBe(true);
  });
});

describe('VideoTile drawing pen in Japanese', () => {
  beforeEach(async () => {
    useUserCache.setState({ users: new Map([['u-x', { id: 'u-x', display_name: 'Ana' } as never]]) });
    useHuddleStore.setState({
      penOn: null,
      participants: {
        'u-x': {
          userId: 'u-x',
          audioStream: null,
          cameraStream: null,
          screenStream: null,
          audioMuted: false,
          cameraOn: false,
          sharing: true,
          drawingEnabled: true,
          handRaised: false,
        },
      },
    });
    await i18n.changeLanguage('ja');
  });
  afterEach(async () => {
    useHuddleStore.setState({ penOn: null });
    await i18n.changeLanguage('en');
  });

  it('names the pen before and while drawing', () => {
    render(tile({ kind: 'screen', tileId: 'u-x:screen', sharing: true, drawing: true }));
    const pen = screen.getByRole('button', { name: 'この画面に描き込む' });
    expect(pen).toHaveAttribute('data-tooltip', 'この画面に描き込む');
    fireEvent.click(pen);
    expect(screen.getByRole('button', { name: '描き込みをやめる（Esc）' })).toHaveAttribute(
      'data-tooltip',
      '描き込みをやめる（Esc）',
    );
  });
});
