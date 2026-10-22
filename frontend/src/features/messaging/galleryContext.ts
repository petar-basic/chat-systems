import { createContext, useContext } from 'react';
import type { components } from '@/api/schema';
import type { Message } from '@/stores/workspace';
import { isRenderableImage } from './fileKinds';

type Attachment = components['schemas']['Attachment'];

export interface GalleryItem {
  attachment: Attachment;
  userId?: string;
  createdAt?: string;
}

export interface GalleryApi {
  open: (attachmentId: string) => void;
}

export const GalleryContext = createContext<GalleryApi | null>(null);

export function useGallery(): GalleryApi | null {
  return useContext(GalleryContext);
}

export function galleryItems(messages: Message[]): GalleryItem[] {
  return messages
    .filter((message) => !message.deleted_at)
    .flatMap((message) =>
      (message.metadata?.attachments ?? [])
        .filter((attachment) => isRenderableImage(attachment.mime_type))
        .map((attachment) => ({ attachment, userId: message.user_id, createdAt: message.created_at })),
    );
}
