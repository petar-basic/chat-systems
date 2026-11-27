import { useCallback } from 'react';
import { create } from 'zustand';
import type { components } from '@/api/schema';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';
import { useInstanceInfo } from '@/features/instance';
import { toast } from '@/shared/components/Toast';
import { toUserMessage } from '@/lib/errors';
import { logger } from '@/lib/logger';
import i18n from '@/lib/i18n';
import { formatBytes } from '@/lib/fileSize';
import { MAX_ATTACHMENTS } from '@/shared/constants';
import type { RecordedClip } from '@/features/clips';

type Uploaded = components['schemas']['FileUploadResponse'];
export type Attachment = components['schemas']['Attachment'];
type Clip = components['schemas']['Clip'];

export interface PendingAttachment {
  localId: string;
  name: string;
  size: number;
  mimeType: string;
  previewUrl: string | null;
  progress: number;
  status: 'uploading' | 'ready' | 'failed';
  error: string | null;
  fileId: string | null;
  url: string | null;
  width: number | null;
  height: number | null;
  clip?: Clip | null;
  posterUrl?: string | null;
  thumbnailUrl?: string | null;
}

export interface ComposerAttachments {
  items: PendingAttachment[];
  busy: boolean;
  failed: boolean;
  hasReady: boolean;
  add: (files: File[]) => void;
  addClip: (clip: RecordedClip) => void;
  remove: (localId: string) => void;
  retry: (localId: string) => void;
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
      ...(item.clip ? { clip: item.clip, thumbnail_url: item.posterUrl ?? item.thumbnailUrl ?? null } : {}),
    }));
}

interface Args {
  workspaceId: string | undefined;
  instanceUrl: string | undefined;
  resetKey: string | undefined;
}

interface Trays {
  trays: Record<string, PendingAttachment[]>;
}

const useTrays = create<Trays>(() => ({ trays: {} }));
const files = new Map<string, File>();
const clips = new Map<string, Pick<RecordedClip, 'poster' | 'clip'>>();
const controllers = new Map<string, AbortController>();
const EMPTY: PendingAttachment[] = [];

function update(scope: string, change: (items: PendingAttachment[]) => PendingAttachment[]) {
  useTrays.setState((state) => ({ trays: { ...state.trays, [scope]: change(state.trays[scope] ?? EMPTY) } }));
}

function patch(scope: string, localId: string, change: Partial<PendingAttachment>) {
  update(scope, (items) => items.map((item) => (item.localId === localId ? { ...item, ...change } : item)));
}

function forget(localId: string) {
  controllers.get(localId)?.abort();
  controllers.delete(localId);
  files.delete(localId);
  clips.delete(localId);
}

export function restoreAttachments(scope: string, attachments: Attachment[]) {
  if (attachments.length === 0) return;
  update(scope, (items) => [
    ...items,
    ...attachments.map((attachment) => ({
      localId: crypto.randomUUID(),
      name: attachment.filename,
      size: attachment.size_bytes,
      mimeType: attachment.mime_type,
      previewUrl: null,
      progress: 1,
      status: 'ready' as const,
      error: null,
      fileId: attachment.id,
      url: attachment.url,
      width: attachment.width ?? null,
      height: attachment.height ?? null,
      clip: attachment.clip ?? null,
      thumbnailUrl: attachment.thumbnail_url ?? null,
    })),
  ]);
}

export function useComposerAttachments({ workspaceId, instanceUrl, resetKey }: Args): ComposerAttachments {
  const scope = resetKey ?? '';
  const items = useTrays((state) => state.trays[scope] ?? EMPTY);
  const { data: info } = useInstanceInfo(instanceUrl);
  const maxBytes = info?.max_upload_bytes;

  const upload = useCallback(
    async (localId: string, file: File) => {
      if (!workspaceId) return;
      const controller = new AbortController();
      controllers.set(localId, controller);
      patch(scope, localId, { status: 'uploading', progress: 0, error: null });
      const recorded = clips.get(localId);
      const form = new FormData();
      if (recorded) {
        form.append('clip', JSON.stringify(recorded.clip));
        if (recorded.poster) form.append('poster', recorded.poster, 'poster.jpg');
        form.append('media', file);
      } else {
        form.append('file', file);
      }
      const onProgress = (progress: number) => patch(scope, localId, { progress });
      const api = getApiForInstance(instanceUrl);
      try {
        const stored = recorded
          ? await api.uploadWithProgress<Uploaded>(
              `/files/clip/${workspaceId}?draft=true`,
              form,
              onProgress,
              controller.signal,
            )
          : (
              await api.uploadWithProgress<Uploaded[]>(
                `/files/upload/${workspaceId}?draft=true`,
                form,
                onProgress,
                controller.signal,
              )
            )[0];
        patch(scope, localId, {
          status: 'ready',
          progress: 1,
          fileId: stored.id,
          url: stored.url,
          width: stored.width ?? null,
          height: stored.height ?? null,
          thumbnailUrl: stored.thumbnail_url ?? null,
        });
        files.delete(localId);
        clips.delete(localId);
      } catch (err) {
        if (err instanceof DOMException && err.name === 'AbortError') return;
        logger.error('useComposerAttachments', 'upload', err);
        const reason = toUserMessage(err, i18n.t('messages:tray.uploadFailed'));
        patch(scope, localId, { status: 'failed', error: reason });
        toast.error(reason);
      } finally {
        controllers.delete(localId);
      }
    },
    [scope, workspaceId, instanceUrl],
  );

  const add = useCallback(
    (picked: File[]) => {
      const room = MAX_ATTACHMENTS - items.length;
      if (picked.length > room) {
        toast.error(`A message can carry at most ${MAX_ATTACHMENTS} files`);
      }
      const accepted = picked.slice(0, Math.max(room, 0)).filter((file) => {
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
          error: null,
          fileId: null,
          url: null,
          width: null,
          height: null,
        },
      }));
      if (fresh.length === 0) return;
      update(scope, (current) => [...current, ...fresh.map(({ item }) => item)]);
      for (const { file, item } of fresh) {
        files.set(item.localId, file);
        void upload(item.localId, file);
      }
    },
    [scope, items.length, maxBytes, upload],
  );

  const addClip = useCallback(
    (recorded: RecordedClip) => {
      if (items.length >= MAX_ATTACHMENTS) {
        toast.error(`A message can carry at most ${MAX_ATTACHMENTS} files`);
        return;
      }
      if (maxBytes !== undefined && recorded.file.size > maxBytes) {
        toast.error(`${recorded.file.name} is larger than ${formatBytes(maxBytes)}`);
        return;
      }
      const item: PendingAttachment = {
        localId: crypto.randomUUID(),
        name: recorded.file.name,
        size: recorded.file.size,
        mimeType: recorded.file.type,
        previewUrl: recorded.previewUrl,
        progress: 0,
        status: 'uploading',
        error: null,
        fileId: null,
        url: null,
        width: null,
        height: null,
        clip: recorded.clip,
        posterUrl: recorded.posterUrl,
        thumbnailUrl: null,
      };
      update(scope, (current) => [...current, item]);
      files.set(item.localId, recorded.file);
      clips.set(item.localId, { poster: recorded.poster, clip: recorded.clip });
      void upload(item.localId, recorded.file);
    },
    [scope, items.length, maxBytes, upload],
  );

  const retry = useCallback(
    (localId: string) => {
      const file = files.get(localId);
      if (file) void upload(localId, file);
    },
    [upload],
  );

  const remove = useCallback(
    (localId: string) => {
      const item = items.find((candidate) => candidate.localId === localId);
      forget(localId);
      update(scope, (current) => current.filter((candidate) => candidate.localId !== localId));
      if (item?.previewUrl) URL.revokeObjectURL(item.previewUrl);
      if (item?.posterUrl) URL.revokeObjectURL(item.posterUrl);
      const fileId = item?.fileId;
      if (fileId) {
        void getApiForInstance(instanceUrl)
          .typed((c) => c.DELETE('/files/{file_id}', { params: { path: { file_id: fileId } } }))
          .catch((err) => logger.error('useComposerAttachments', 'remove', err));
      }
    },
    [scope, items, instanceUrl],
  );

  const take = useCallback(() => {
    const ready = items.filter((item) => item.status === 'ready');
    for (const item of items) forget(item.localId);
    update(scope, () => EMPTY);
    return ready;
  }, [scope, items]);

  return {
    items,
    busy: items.some((item) => item.status === 'uploading'),
    failed: items.some((item) => item.status === 'failed'),
    hasReady: items.some((item) => item.status === 'ready'),
    add,
    addClip,
    remove,
    retry,
    take,
  };
}
