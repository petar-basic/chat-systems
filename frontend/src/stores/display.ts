import { create } from 'zustand';
import type { components } from '@/api/schema';

export type Density = components['schemas']['Density'];
export type Previews = components['schemas']['Previews'];

interface DisplayState {
  density: Density;
  previews: Previews;
  show: (next: { density: Density; previews: Previews }) => void;
}

export const useDisplayStore = create<DisplayState>((set) => ({
  density: 'clean',
  previews: 'shown',
  show: (next) => set(next),
}));
