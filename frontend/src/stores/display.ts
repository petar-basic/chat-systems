import { create } from 'zustand';
import type { components } from '@/api/schema';

export type Density = components['schemas']['Density'];
export type Previews = components['schemas']['Previews'];
export type EnterKey = components['schemas']['EnterKey'];
export type OpenChannelAt = components['schemas']['OpenChannelAt'];

interface Display {
  density: Density;
  previews: Previews;
  enterKey: EnterKey;
  openChannelAt: OpenChannelAt;
}

interface DisplayState extends Display {
  show: (next: Display) => void;
}

export const useDisplayStore = create<DisplayState>((set) => ({
  density: 'clean',
  previews: 'shown',
  enterKey: 'send',
  openChannelAt: 'where_left',
  show: (next) => set(next),
}));
