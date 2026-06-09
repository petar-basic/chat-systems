import { useEffect, useState } from 'react';
import { logger } from '@/lib/logger';

export interface MediaDeviceLists {
  mics: MediaDeviceInfo[];
  cameras: MediaDeviceInfo[];
  speakers: MediaDeviceInfo[];
}

const EMPTY: MediaDeviceLists = { mics: [], cameras: [], speakers: [] };

export function useMediaDevices(enabled: boolean): MediaDeviceLists {
  const [devices, setDevices] = useState<MediaDeviceLists>(EMPTY);

  useEffect(() => {
    if (!enabled) return;
    let cancelled = false;

    const refresh = async () => {
      try {
        const all = await navigator.mediaDevices.enumerateDevices();
        if (cancelled) return;
        setDevices({
          mics: all.filter((d) => d.kind === 'audioinput'),
          cameras: all.filter((d) => d.kind === 'videoinput'),
          speakers: all.filter((d) => d.kind === 'audiooutput'),
        });
      } catch (err) {
        logger.error('useMediaDevices', 'enumerateDevices', err);
      }
    };

    void refresh();
    navigator.mediaDevices.addEventListener('devicechange', refresh);
    return () => {
      cancelled = true;
      navigator.mediaDevices.removeEventListener('devicechange', refresh);
    };
  }, [enabled]);

  return devices;
}
