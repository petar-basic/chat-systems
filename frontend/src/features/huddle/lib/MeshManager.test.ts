import { describe, it, expect } from 'vitest';
import { slotOf } from './MeshManager';

const transceiver = (kind: 'audio' | 'video') =>
  ({ receiver: { track: { kind } } }) as unknown as RTCRtpTransceiver;

describe('naming what a mesh peer sends', () => {
  const own = { audio: transceiver('audio'), camera: transceiver('video'), screen: transceiver('video') };
  const theirs = [transceiver('audio'), transceiver('video'), transceiver('video')];

  it('reads the peer’s own m-lines in the order every end adds them: microphone, camera, screen', () => {
    const all = [own.audio, own.camera, own.screen, ...theirs];
    expect(theirs.map((t) => slotOf(t, own, all))).toEqual(['audio', 'camera', 'screen']);
  });

  it('reads them the same when the peer offered first', () => {
    const all = [...theirs, own.audio, own.camera, own.screen];
    expect(theirs.map((t) => slotOf(t, own, all))).toEqual(['audio', 'camera', 'screen']);
  });

  it('names a slot of ours by itself if the browser lent it to the peer', () => {
    const all = [own.audio, own.camera, own.screen];
    expect([own.audio, own.camera, own.screen].map((t) => slotOf(t, own, all))).toEqual([
      'audio',
      'camera',
      'screen',
    ]);
  });
});
