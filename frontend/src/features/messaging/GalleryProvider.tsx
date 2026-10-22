import { useMemo, useState, type ReactNode } from 'react';
import type { Message } from '@/stores/workspace';
import { GalleryContext, galleryItems } from './galleryContext';
import { ImageGallery } from './ImageGallery';

interface Props {
  messages: Message[];
  children: ReactNode;
}

export function GalleryProvider({ messages, children }: Props) {
  const items = useMemo(() => galleryItems(messages), [messages]);
  const [openId, setOpenId] = useState<string | null>(null);
  const api = useMemo(() => ({ open: setOpenId }), []);
  const index = openId ? items.findIndex((item) => item.attachment.id === openId) : -1;

  return (
    <GalleryContext.Provider value={api}>
      {children}
      {index >= 0 && (
        <ImageGallery
          items={items}
          index={index}
          onIndexChange={(next) => setOpenId(items[next]?.attachment.id ?? null)}
          onClose={() => setOpenId(null)}
        />
      )}
    </GalleryContext.Provider>
  );
}
