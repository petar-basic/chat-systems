import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { act, fireEvent, render, screen } from '@testing-library/react';
import { acquireCamera, acquireLocalAudio, acquireScreen, stopStream } from '@/lib/media';
import { CLIP_MAX_MS } from '../lib/format';
import { posterOf } from '../lib/poster';
import { makeSeekable } from '../lib/remux';
import i18n from '@/lib/i18n';
import ClipRecorderDialog from './ClipRecorderDialog';
import { ClipButton } from './ClipButton';

vi.mock('@/lib/media', async (original) => ({
  ...(await original<typeof import('@/lib/media')>()),
  acquireLocalAudio: vi.fn(),
  acquireCamera: vi.fn(),
  acquireScreen: vi.fn(),
  stopStream: vi.fn(),
}));
vi.mock('../lib/remux', () => ({ makeSeekable: vi.fn(async (blob: Blob) => ({ blob, durationMs: 4200 })) }));
vi.mock('../lib/poster', () => ({
  posterOf: vi.fn(async () => new Blob(['poster'], { type: 'image/jpeg' })),
}));
vi.mock('../lib/ticker', () => ({ tick: vi.fn(() => () => undefined) }));

class FakeTrack {
  stop = vi.fn();
  addEventListener = vi.fn();
  constructor(readonly kind: 'audio' | 'video') {}
  getSettings() {
    return { width: 1280, height: 720 };
  }
}

class FakeStream {
  constructor(private readonly tracks: FakeTrack[] = []) {}
  getTracks() {
    return this.tracks;
  }
  getAudioTracks() {
    return this.tracks.filter((track) => track.kind === 'audio');
  }
  getVideoTracks() {
    return this.tracks.filter((track) => track.kind === 'video');
  }
}

class FakeRecorder {
  static made: FakeRecorder[] = [];
  static isTypeSupported = (type: string) => type.includes('webm');
  state = 'inactive';
  mimeType: string;
  ondataavailable: ((event: { data: Blob }) => void) | null = null;
  onstop: (() => void) | null = null;
  onerror: (() => void) | null = null;
  constructor(
    readonly stream: FakeStream,
    options: { mimeType: string },
  ) {
    this.mimeType = options.mimeType;
    FakeRecorder.made.push(this);
  }
  start() {
    this.state = 'recording';
  }
  stop() {
    this.state = 'inactive';
    this.ondataavailable?.({ data: new Blob(['recorded'], { type: this.mimeType }) });
    this.onstop?.();
  }
}

const microphone = () => new FakeStream([new FakeTrack('audio')]);
const camera = () => new FakeStream([new FakeTrack('video')]);
const denied = (name: string) => new DOMException('denied', name);
const flush = () => act(async () => {});

function open(onClip = vi.fn()) {
  const onClose = vi.fn();
  render(<ClipRecorderDialog onClip={onClip} onClose={onClose} />);
  return { onClip, onClose };
}

function share(screenShare: boolean) {
  Object.defineProperty(navigator, 'mediaDevices', {
    configurable: true,
    value: screenShare ? { getUserMedia: vi.fn(), getDisplayMedia: vi.fn() } : { getUserMedia: vi.fn() },
  });
}

beforeEach(() => {
  FakeRecorder.made = [];
  vi.stubGlobal('MediaRecorder', FakeRecorder);
  vi.stubGlobal('MediaStream', FakeStream);
  URL.createObjectURL = vi.fn(() => 'blob:clip');
  URL.revokeObjectURL = vi.fn();
  share(false);
  vi.mocked(acquireLocalAudio).mockReset();
  vi.mocked(acquireCamera).mockReset();
  vi.mocked(acquireScreen).mockReset();
  vi.mocked(stopStream).mockReset();
  vi.mocked(posterOf).mockClear();
});

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

describe('recording a clip', () => {
  it('records the voice, shows it back and hands it over with its waveform', async () => {
    vi.mocked(acquireLocalAudio).mockResolvedValue(microphone() as unknown as MediaStream);
    const { onClip, onClose } = open();

    fireEvent.click(screen.getByTestId('clip-mode-audio'));
    fireEvent.click(screen.getByTestId('clip-start'));
    await flush();
    expect(screen.getByTestId('clip-clock')).toHaveTextContent('0:00 of 5:00');
    expect(FakeRecorder.made[0].mimeType).toBe('audio/webm;codecs=opus');
    expect(acquireCamera).not.toHaveBeenCalled();

    fireEvent.click(screen.getByTestId('clip-stop'));
    await flush();
    expect(screen.getByTestId('clip-player')).toHaveAttribute('data-kind', 'audio');
    expect(screen.getByTestId('clip-duration')).toHaveTextContent('0:00 / 0:04');

    fireEvent.click(screen.getByTestId('clip-use'));
    const [[clip]] = onClip.mock.calls;
    expect(clip.clip).toEqual({ kind: 'audio', duration_ms: 4200, waveform: Array(100).fill(0) });
    expect(clip.poster).toBeNull();
    expect(clip.file.type).toBe('audio/webm');
    expect(clip.file.name).toMatch(/^Clip \d{4}-\d\d-\d\d \d\d\.\d\d\.\d\d\.webm$/);
    expect(onClose).toHaveBeenCalled();
    expect(posterOf).not.toHaveBeenCalled();
  });

  it('gives a camera clip a poster and lets it be recorded again', async () => {
    vi.mocked(acquireLocalAudio).mockResolvedValue(microphone() as unknown as MediaStream);
    vi.mocked(acquireCamera).mockResolvedValue(camera() as unknown as MediaStream);
    const { onClip } = open();

    fireEvent.click(screen.getByTestId('clip-start'));
    await flush();
    expect(screen.getByTestId('clip-live')).toBeInTheDocument();
    fireEvent.click(screen.getByTestId('clip-stop'));
    await flush();
    expect(screen.getByTestId('clip-player')).toHaveAttribute('data-kind', 'video');

    fireEvent.click(screen.getByTestId('clip-again'));
    expect(screen.getByTestId('clip-start')).toBeInTheDocument();
    expect(URL.revokeObjectURL).toHaveBeenCalled();

    fireEvent.click(screen.getByTestId('clip-start'));
    await flush();
    fireEvent.click(screen.getByTestId('clip-stop'));
    await flush();
    fireEvent.click(screen.getByTestId('clip-use'));
    const [[clip]] = onClip.mock.calls;
    expect(clip.clip).toEqual({ kind: 'video', duration_ms: 4200 });
    expect(clip.poster).toBeInstanceOf(Blob);
    expect(clip.file.type).toBe('video/webm');
  });

  it('stops by itself at five minutes, counting down the last thirty seconds', async () => {
    vi.useFakeTimers({
      toFake: ['setTimeout', 'clearTimeout', 'setInterval', 'clearInterval', 'performance', 'Date'],
    });
    vi.mocked(acquireLocalAudio).mockResolvedValue(microphone() as unknown as MediaStream);
    open();
    fireEvent.click(screen.getByTestId('clip-mode-audio'));
    fireEvent.click(screen.getByTestId('clip-start'));
    await act(() => vi.advanceTimersByTimeAsync(0));
    expect(screen.queryByTestId('clip-countdown')).not.toBeInTheDocument();

    await act(() => vi.advanceTimersByTimeAsync(CLIP_MAX_MS - 29_000));
    expect(screen.getByTestId('clip-countdown')).toHaveTextContent('0:29 left');
    expect(FakeRecorder.made[0].state).toBe('recording');

    await act(() => vi.advanceTimersByTimeAsync(29_000));
    expect(FakeRecorder.made[0].state).toBe('inactive');
    expect(screen.getByTestId('clip-use')).toBeInTheDocument();
  });
});

describe('a clip nobody allowed', () => {
  it('says the microphone was blocked and records nothing', async () => {
    vi.mocked(acquireLocalAudio).mockRejectedValue(denied('NotAllowedError'));
    const { onClip } = open();
    fireEvent.click(screen.getByTestId('clip-start'));
    await flush();
    expect(screen.getByTestId('clip-error')).toHaveTextContent(
      'Your browser blocked the microphone. Allow it for this site and try again.',
    );
    expect(acquireCamera).not.toHaveBeenCalled();
    expect(FakeRecorder.made).toHaveLength(0);
    expect(onClip).not.toHaveBeenCalled();
  });

  it('says sharing the screen was not allowed before asking for anything else', async () => {
    share(true);
    vi.mocked(acquireScreen).mockRejectedValue(denied('NotAllowedError'));
    open();
    expect(screen.getByTestId('clip-mode-screen')).toBeInTheDocument();
    fireEvent.click(screen.getByTestId('clip-start'));
    await flush();
    expect(screen.getByTestId('clip-error')).toHaveTextContent(
      'Sharing your screen was not allowed, so nothing was recorded.',
    );
    expect(acquireLocalAudio).not.toHaveBeenCalled();
    expect(FakeRecorder.made).toHaveLength(0);
  });

  it('lets go of the microphone when the camera is blocked', async () => {
    const mic = microphone();
    vi.mocked(acquireLocalAudio).mockResolvedValue(mic as unknown as MediaStream);
    vi.mocked(acquireCamera).mockRejectedValue(denied('NotAllowedError'));
    open();
    fireEvent.click(screen.getByTestId('clip-start'));
    await flush();
    expect(screen.getByTestId('clip-error')).toHaveTextContent(
      'Your browser blocked the camera. Allow it for this site and try again.',
    );
    expect(stopStream).toHaveBeenCalledWith(mic);
    expect(FakeRecorder.made).toHaveLength(0);

    fireEvent.click(screen.getByTestId('clip-retry'));
    expect(screen.getByTestId('clip-start')).toBeInTheDocument();
  });
});

describe('recording a clip in Japanese', () => {
  beforeEach(async () => {
    await i18n.changeLanguage('ja');
  });

  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('names the clip button', () => {
    share(true);
    render(<ClipButton onClip={vi.fn()} />);
    const button = screen.getByTestId('composer-clip');
    expect(button).toHaveAttribute('aria-label', 'クリップを収録');
    expect(button).toHaveAttribute('data-tooltip', 'クリップを収録');
  });

  it('labels each thing to record, and the bubble on a screen recording', () => {
    share(true);
    open();
    expect(screen.getByText('クリップを収録')).toBeInTheDocument();
    expect(screen.getByText('収録する内容')).toBeInTheDocument();
    for (const [mode, label, hint] of [
      ['audio', '音声', 'あなたの声だけ'],
      ['camera', 'カメラ', 'カメラに映るあなたと、あなたの声'],
      ['screen', '画面', 'あなたの画面と、あなたの声'],
    ]) {
      expect(screen.getByTestId(`clip-mode-${mode}`)).toHaveTextContent(`${label}${hint}`);
    }
    expect(screen.getByLabelText('自分のカメラ映像を丸窓で表示')).toBeInTheDocument();
    expect(screen.getByText('画面収録の隅に表示されます')).toBeInTheDocument();
    expect(screen.getByTestId('clip-start')).toHaveTextContent('収録を開始');
  });

  it('counts the time, counts down the end, gets the clip ready and offers it', async () => {
    vi.useFakeTimers({
      toFake: ['setTimeout', 'clearTimeout', 'setInterval', 'clearInterval', 'performance', 'Date'],
    });
    vi.mocked(acquireLocalAudio).mockResolvedValue(microphone() as unknown as MediaStream);
    let seekable: (value: { blob: Blob; durationMs: number }) => void = () => undefined;
    vi.mocked(makeSeekable).mockImplementationOnce(
      (blob: Blob) =>
        new Promise((resolve) => {
          seekable = () => resolve({ blob, durationMs: 4200 });
        }),
    );
    open();
    fireEvent.click(screen.getByTestId('clip-mode-audio'));
    fireEvent.click(screen.getByTestId('clip-start'));
    await act(() => vi.advanceTimersByTimeAsync(0));
    expect(screen.getByTestId('clip-clock')).toHaveTextContent('0:00 / 5:00');
    expect(screen.getByTestId('clip-stop')).toHaveTextContent('収録を停止');

    await act(() => vi.advanceTimersByTimeAsync(CLIP_MAX_MS - 29_000));
    expect(screen.getByTestId('clip-countdown')).toHaveTextContent('残り 0:29');

    fireEvent.click(screen.getByTestId('clip-stop'));
    await act(() => vi.advanceTimersByTimeAsync(0));
    expect(screen.getByRole('status', { name: 'クリップを準備しています…' })).toBeInTheDocument();

    seekable({ blob: new Blob(), durationMs: 4200 });
    await act(() => vi.advanceTimersByTimeAsync(0));
    expect(screen.getByTestId('clip-use')).toHaveTextContent('このクリップを使う');
    expect(screen.getByTestId('clip-again')).toHaveTextContent('もう一度収録');
    expect(
      screen.getByText('クリップはメッセージに添付されます。必要なら文章を添えてから送信してください。'),
    ).toBeInTheDocument();
  });

  it.each([
    [
      'the camera is blocked',
      'NotAllowedError',
      'ブラウザがカメラをブロックしています。このサイトでカメラの使用を許可してから、もう一度お試しください。',
    ],
    [
      'there is no camera',
      'NotFoundError',
      'カメラが見つかりません。カメラを接続してから、もう一度お試しください。',
    ],
  ])('says when %s, and offers to try again', async (_, name, message) => {
    vi.mocked(acquireLocalAudio).mockResolvedValue(microphone() as unknown as MediaStream);
    vi.mocked(acquireCamera).mockRejectedValue(denied(name));
    open();
    fireEvent.click(screen.getByTestId('clip-start'));
    await flush();
    expect(screen.getByTestId('clip-error').textContent).toBe(message);
    expect(screen.getByTestId('clip-retry')).toHaveTextContent('再試行');
  });

  it('says when sharing the screen was not allowed', async () => {
    share(true);
    vi.mocked(acquireScreen).mockRejectedValue(denied('NotAllowedError'));
    open();
    fireEvent.click(screen.getByTestId('clip-start'));
    await flush();
    expect(screen.getByTestId('clip-error').textContent).toBe(
      '画面の共有が許可されなかったため、何も収録されていません。',
    );
  });

  it('says when the browser cannot record', async () => {
    FakeRecorder.isTypeSupported = () => false;
    try {
      open();
      fireEvent.click(screen.getByTestId('clip-start'));
      await flush();
      expect(screen.getByTestId('clip-error').textContent).toBe(
        'このブラウザでは、クリップを収録できません。',
      );
    } finally {
      FakeRecorder.isTypeSupported = (type: string) => type.includes('webm');
    }
  });

  it('says when the recording failed', async () => {
    vi.mocked(acquireLocalAudio).mockResolvedValue(microphone() as unknown as MediaStream);
    open();
    fireEvent.click(screen.getByTestId('clip-mode-audio'));
    fireEvent.click(screen.getByTestId('clip-start'));
    await flush();
    const [recorder] = FakeRecorder.made;
    recorder.stop = () => recorder.onerror?.();
    fireEvent.click(screen.getByTestId('clip-stop'));
    await flush();
    expect(screen.getByTestId('clip-error').textContent).toBe(
      '収録中にエラーが起きたため、何も保存されていません。',
    );
  });
});

describe('the clip strings', () => {
  function leaves(value: unknown, prefix = ''): string[] {
    if (typeof value !== 'object' || value === null) return [prefix];
    return Object.entries(value).flatMap(([key, child]) => leaves(child, prefix ? `${prefix}.${key}` : key));
  }

  it('has Japanese for every one', () => {
    const english = leaves(i18n.getResource('en', 'messages', 'clips'));
    const japanese = leaves(i18n.getResource('ja', 'messages', 'clips'));
    expect(english.length).toBeGreaterThan(0);
    expect(japanese).toEqual(english);
  });
});
