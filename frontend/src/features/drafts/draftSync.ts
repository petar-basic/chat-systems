import { getApiForInstance } from '@/shared/hooks/useCurrentApi';
import { ApiError } from '@/lib/errors';
import {
  draftKeyOf,
  ownDrafts,
  useDraftStore,
  type DraftPlace,
  type LocalDraft,
  type SavedDraft,
} from '@/stores/drafts';

export const DRAFT_SYNC_DELAY_MS = 1200;

const timers = new Map<string, ReturnType<typeof setTimeout>>();
const inFlight = new Map<string, Promise<void>>();
const again = new Set<string>();
const settledLately = new Set<string>();

const refused = (error: unknown) =>
  error instanceof ApiError &&
  error.status >= 400 &&
  error.status < 500 &&
  error.status !== 401 &&
  error.status !== 429;

async function upload(place: DraftPlace, draft: LocalDraft): Promise<SavedDraft> {
  return getApiForInstance(place.instanceUrl).typed((c) =>
    c.PUT('/workspaces/{ws_id}/drafts', {
      params: { path: { ws_id: place.workspaceId } },
      body: {
        channel_id: place.channelId,
        thread_parent_id: place.threadParentId,
        content: draft.text,
        files_left_behind: !!draft.filesLeftBehind,
        updated_at: draft.at,
      },
    }),
  );
}

async function send(key: string): Promise<void> {
  const draft = useDraftStore.getState().entry(key);
  if (!draft?.place || !draft.pending) return;
  try {
    const saved = await upload(draft.place, draft);
    useDraftStore.getState().settle(key, draft.at, saved);
    settledLately.add(key);
  } catch (error) {
    if (refused(error)) useDraftStore.getState().keepLocal(key);
  }
}

export function flushDraft(key: string): Promise<void> {
  const running = inFlight.get(key);
  if (running) {
    again.add(key);
    return running;
  }
  const run = send(key).finally(() => {
    inFlight.delete(key);
    if (again.delete(key)) void flushDraft(key);
  });
  inFlight.set(key, run);
  return run;
}

export function scheduleDraftFlush(key: string) {
  clearTimeout(timers.get(key));
  timers.set(
    key,
    setTimeout(() => {
      timers.delete(key);
      void flushDraft(key);
    }, DRAFT_SYNC_DELAY_MS),
  );
}

export function watchDraftChanges(): () => void {
  return useDraftStore.subscribe((now, before) => {
    if (now.drafts === before.drafts) return;
    const previous = new Map(ownDrafts(before.drafts));
    for (const [key, draft] of ownDrafts(now.drafts)) {
      if (draft.pending && draft.place && previous.get(key) !== draft) scheduleDraftFlush(key);
    }
  });
}

export async function reconcileDrafts(instanceUrl: string, workspaceId: string): Promise<void> {
  const waiting = ownDrafts(useDraftStore.getState().drafts)
    .filter(([, draft]) => draft.pending && draft.place?.workspaceId === workspaceId)
    .map(([key]) => flushDraft(key));
  await Promise.all(waiting);
  settledLately.clear();
  const res = await getApiForInstance(instanceUrl).typed((c) =>
    c.GET('/workspaces/{ws_id}/drafts', { params: { path: { ws_id: workspaceId } } }),
  );
  const listed = new Set(settledLately);
  for (const saved of res.data) {
    useDraftStore.getState().accept(instanceUrl, saved);
    listed.add(draftKeyOf({ channelId: saved.channel_id, threadParentId: saved.thread_parent_id ?? null }));
  }
  useDraftStore.getState().forgetMissing(workspaceId, listed);
}
