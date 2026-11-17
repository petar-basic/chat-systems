import { describe, it, expect } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import { ClipPlayer } from './ClipPlayer';

const peaks = Array.from({ length: 100 }, (_, i) => i);

describe('ClipPlayer', () => {
  it('plays an audio clip over its waveform at one, one and a half and two times', () => {
    render(
      <ClipPlayer
        url="https://chat.test/api/files/download/ws/1/Clip.m4a"
        clip={{ kind: 'audio', duration_ms: 12_000, waveform: peaks }}
        filename="Clip.m4a"
      />,
    );
    const audio = document.querySelector('audio');
    if (!audio) throw new Error('no audio element');
    const waveform = screen.getByTestId('clip-waveform');
    expect(waveform.children).toHaveLength(100);
    expect(waveform).toHaveAttribute('aria-valuemax', '12');
    expect(screen.getByTestId('clip-duration')).toHaveTextContent('0:00 / 0:12');

    const speed = screen.getByTestId('clip-speed');
    expect(speed).toHaveTextContent('1×');
    fireEvent.click(speed);
    expect(speed).toHaveTextContent('1.5×');
    expect(audio.playbackRate).toBe(1.5);
    fireEvent.click(speed);
    expect(audio.playbackRate).toBe(2);
    fireEvent.click(speed);
    expect(speed).toHaveTextContent('1×');

    fireEvent.keyDown(waveform, { key: 'ArrowRight' });
    expect(audio.currentTime).toBe(5);
    fireEvent.keyDown(waveform, { key: 'End' });
    expect(audio.currentTime).toBe(12);
  });

  it('shows a video clip with its poster, in its own shape', () => {
    render(
      <ClipPlayer
        url="https://chat.test/api/files/download/ws/1/Clip.mp4"
        posterUrl="https://chat.test/api/files/download/ws/1/poster.jpg"
        clip={{ kind: 'video', duration_ms: 20_000 }}
        filename="Clip.mp4"
        width={1280}
        height={800}
      />,
    );
    const video = document.querySelector('video');
    expect(video).toHaveAttribute('poster', 'https://chat.test/api/files/download/ws/1/poster.jpg');
    expect(video).toHaveAttribute('preload', 'metadata');
    expect(video?.style.aspectRatio).toBe('1280 / 800');
    expect(screen.getByTestId('clip-duration')).toHaveTextContent('0:20');
    expect(screen.queryByTestId('clip-expand')).not.toBeInTheDocument();
  });
});
