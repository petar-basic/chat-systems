import { create } from 'zustand';
import { foldKeyword } from '@/lib/keywords';

interface KeywordState {
  folded: string[];
  populate: (keywords: string[]) => void;
}

export const useKeywordStore = create<KeywordState>((set) => ({
  folded: [],
  populate: (keywords) => set({ folded: keywords.map(foldKeyword).sort((a, b) => b.length - a.length) }),
}));
