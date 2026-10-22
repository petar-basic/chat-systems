import { useCallback, useEffect, useRef, useState } from 'react';
import type { components } from '@/api/schema';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';
import { useInstanceInfo } from '@/hooks/queries/useInstanceInfo';
import { toast } from '@/shared/components/Toast';
import { toUserMessage } from '@/lib/errors';
import { logger } from '@/lib/logger';
import { formatBytes } from '@/lib/fileSize';
import { ErrorLabels, MAX_ATTACHMENTS } from '@/shared/constants';

type Uploaded = components['schemas']['FileUploadResponse'];
export type Attachment = components['schemas']['Attachment'];

export interface PendingAttachment {
  localId: string;
  name: string;
  size: number;
  mimeType: string;
  previewUrl: string | null;
  progress: number;
  status: 'uploading' | 'ready' | 'failed';
  fileId: string | null;
  url: string | null;
  width: number | null;
  height: number | null;
}

export interface ComposerAttachments {
  items: PendingAttachment[];
  busy: boolean;
  hasReady: boolean;
  add: (files: File[]) => void;
  remove: (localId: string) => void;
  take: () => PendingAttachment[];
}

export function asAttachments(items: PendingAttachment[]): Attachment[] {
  return items
    .filter((item) => item.status === 'ready' && item.fileId)
    .map((item) => ({
      id: item.fileId ?? '',
      filename: item.name,
      mime_type: item.mimeType,
      size_bytes: item.size,
      url: item.previewUrl ?? item.url ?? '',
      width: item.width,
      height: item.height,
    }));
}

interface Args {
  workspaceId: string | undefined;
  instanceUrl: string | undefined;
  resetKey: string | undefined;
}

export function useComposerAttachments({ workspaceId, instanceUrl, resetKey }: Args): ComposerAttachments {
  const [items, setItems] = useState<PendingAttachment[]>([]);
  const [trackedKey, setTrackedKey] = useState(resetKey);
  const controllers = useRef(new Map<string, AbortController>());
  const { data: info } = useInstanceInfo(instanceUrl);
  const maxBytes = info?.max_upload_bytes;

  if (trackedKey !== resetKey) {
    setTrackedKey(resetKey);
    setItems([]);
  }

  useEffect(() => {
    const pending = controllers.current;
    return () => {
      for (const controller of pending.values()) controller.abort();
      pending.clear();
    };
  }, [resetKey]);

  const patch = useCallback((localId: string, change: Partial<PendingAttachment>) => {
    setItems((current) => current.map((item) => (item.localId === localId ? { ...item, ...change } : item)));
  }, []);

  const upload = useCallback(
    async (localId: string, file: File) => {
      if (!workspaceId) return;
      const controller = new AbortController();
      controllers.current.set(localId, controller);
      const form = new FormData();
      form.append('file', file);
      try {
        const [stored] = await getApiForInstance(instanceUrl).uploadWithProgress<Uploaded[]>(
          `/files/upload/${workspaceId}?draft=true`,
          form,
          (progress) => patch(localId, { progress }),
          controller.signal,
        );
        patch(localId, {
          status: 'ready',
          progress: 1,
          fileId: stored.id,
          url: stored.url,
          width: stored.width ?? null,
          height: stored.height ?? null,
        });
      } catch (err) {
        if (err instanceof DOMException && err.name === 'AbortError') return;
        logger.error('useComposerAttachments', 'upload', err);
        patch(localId, { status: 'failed' });
        toast.error(toUserMessage(err, ErrorLabels.UploadFailed));
      } finally {
        controllers.current.delete(localId);
      }
    },
    [workspaceId, instanceUrl, patch],
  );

  const add = useCallback(
    (files: File[]) => {
      const room = MAX_ATTACHMENTS - items.length;
      if (files.length > room) {
        toast.error(`A message can carry at most ${MAX_ATTACHMENTS} files`);
      }
      const accepted = files.slice(0, Math.max(room, 0)).filter((file) => {
        if (maxBytes !== undefined && file.size > maxBytes) {
          toast.error(`${file.name} is larger than ${formatBytes(maxBytes)}`);
          return false;
        }
        return true;
      });
      const fresh = accepted.map((file) => ({
        file,
        item: {
          localId: crypto.randomUUID(),
          name: file.name,
          size: file.size,
          mimeType: file.type || 'application/octet-stream',
          previewUrl: file.type.startsWith('image/') ? URL.createObjectURL(file) : null,
          progress: 0,
          status: 'uploading' as const,
          fileId: null,
          url: null,
          width: null,
          height: null,
        },
      }));
      if (fresh.length === 0) return;
      setItems((current) => [...current, ...fresh.map(({ item }) => item)]);
      for (const { file, item } of fresh) void upload(item.localId, file);
    },
    [items.length, maxBytes, upload],
  );

  const remove = useCallback(
    (localId: string) => {
      const item = items.find((candidate) => candidate.localId === localId);
      controllers.current.get(localId)?.abort();
      setItems((current) => current.filter((candidate) => candidate.localId !== localId));
      if (item?.previewUrl) URL.revokeObjectURL(item.previewUrl);
      const fileId = item?.fileId;
      if (fileId) {
        void getApiForInstance(instanceUrl)
          .typed((c) => c.DELETE('/files/{file_id}', { params: { path: { file_id: fileId } } }))
          .catch((err) => logger.error('useComposerAttachments', 'remove', err));
      }
    },
    [items, instanceUrl],
  );

  const take = useCallback(() => {
    const ready = items.filter((item) => item.status === 'ready');
    setItems([]);
    return ready;
  }, [items]);

  return {
    items,
    busy: items.some((item) => item.status === 'uploading'),
    hasReady: items.some((item) => item.status === 'ready'),
    add,
    remove,
    take,
  };
}
