import { useEffect, useState } from 'react';

const SPEAKING_THRESHOLD = 18;
const DEBOUNCE_MS = 250;

export function useSpeaking(stream: MediaStream | null): boolean {
  const [speaking, setSpeaking] = useState(false);

  useEffect(() => {
    if (!stream || stream.getAudioTracks().length === 0) return;

    const AudioCtx =
      window.AudioContext ?? (window as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
    if (!AudioCtx) return;

    const ctx = new AudioCtx();
    const source = ctx.createMediaStreamSource(stream);
    const analyser = ctx.createAnalyser();
    analyser.fftSize = 512;
    source.connect(analyser);

    const data = new Uint8Array(analyser.frequencyBinCount);
    let raf = 0;
    let current = false;
    let lastChange = 0;

    const tick = () => {
      analyser.getByteFrequencyData(data);
      let sum = 0;
      for (const value of data) sum += value;
      const avg = sum / data.length;
      const now = performance.now();
      const next = avg > SPEAKING_THRESHOLD;
      if (next !== current && now - lastChange > DEBOUNCE_MS) {
        current = next;
        lastChange = now;
        setSpeaking(next);
      }
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);

    return () => {
      cancelAnimationFrame(raf);
      source.disconnect();
      void ctx.close();
    };
  }, [stream]);

  return speaking;
}
