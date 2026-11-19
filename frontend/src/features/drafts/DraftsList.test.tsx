import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { act, fireEvent, render, screen } from '@testing-library/react';
import { MemoryRouter, Route, Routes, useLocation } from 'react-router';
import type { Channel } from '@/stores/workspace';
import type { Conversation } from '@/features/messaging';
import { REMOTE_DRAFT_EVENT, useDraftStore, type DraftPlace } from '@/stores/drafts';
import i18n from '@/lib/i18n';
import { DraftsList } from './DraftsList';

const hostile = '<img src=x onerror="alert(1)"> {{name}}';

const URL = 'https://chat.example';
const place = (
  channelId: string,
  threadParentId: string | null = null,
  workspaceId = 'ws-1',
): DraftPlace => ({
  instanceUrl: URL,
  workspaceId,
  channelId,
  threadParentId,
});
const channels = [{ id: 'ch-1', name: 'design' }] as unknown as Channel[];
const conversations = [
  { id: 'dm-1', kind: 'direct', participant_ids: ['u-1', 'u-2'], departed: [] },
] as unknown as Conversation[];

function Where() {
  return <p data-qa="where">{useLocation().pathname}</p>;
}

function renderList(onOpenThread = vi.fn(), onOpened = vi.fn()) {
  render(
    <MemoryRouter initialEntries={['/app/ws-1']}>
      <Routes>
        <Route
          path="*"
          element={
            <>
              <DraftsList
                workspaceId="ws-1"
                channels={channels}
                conversations={conversations}
                onOpenThread={onOpenThread}
                onOpened={onOpened}
              />
              <Where />
            </>
          }
        />
      </Routes>
    </MemoryRouter>,
  );
  return { onOpenThread, onOpened };
}

describe('DraftsList', () => {
  beforeEach(() => {
    vi.useRealTimers();
    useDraftStore.setState({ drafts: {} });
  });

  it('lists this workspace’s drafts, newest first, and nothing kept only on this device', () => {
    vi.useFakeTimers({ now: new Date('2026-11-14T09:00:00Z') });
    useDraftStore.getState().setDraft('ch-1', 'in **design**', { place: place('ch-1') });
    vi.setSystemTime(new Date('2026-11-14T10:00:00Z'));
    useDraftStore
      .getState()
      .setDraft('ch-1/m-1', 'a reply', { place: place('ch-1', 'm-1'), filesLeftBehind: true });
    useDraftStore.getState().setDraft('ch-9', 'elsewhere', { place: place('ch-9', null, 'ws-2') });
    useDraftStore.getState().setDraft('list-comment:i-1', 'a comment');
    vi.useRealTimers();

    renderList();

    const rows = screen.getAllByTestId('draft-row');
    expect(rows.map((row) => row.querySelector('[data-qa="draft-content"]')?.textContent)).toEqual([
      'a reply',
      'in design',
    ]);
    expect(rows[0]).toHaveTextContent('#design · thread');
    expect(rows[0].querySelector('[data-qa="draft-files-left"]')).not.toBeNull();
  });

  it('opens a channel draft in its channel and a thread draft in its thread', () => {
    vi.useFakeTimers({ now: new Date('2026-11-14T09:00:00Z') });
    useDraftStore.getState().setDraft('dm-1', 'see you', { place: place('dm-1') });
    vi.setSystemTime(new Date('2026-11-14T10:00:00Z'));
    useDraftStore.getState().setDraft('ch-1/m-1', 'a reply', { place: place('ch-1', 'm-1') });
    vi.useRealTimers();
    const { onOpenThread, onOpened } = renderList();

    const [thread, dm] = screen.getAllByTestId('draft-open');
    fireEvent.click(dm);
    expect(screen.getByTestId('where')).toHaveTextContent('/app/ws-1/c/dm-1');
    expect(onOpened).toHaveBeenCalled();
    fireEvent.click(thread);
    expect(onOpenThread).toHaveBeenCalledWith('m-1');
  });

  it('deletes a draft and empties its open composer', () => {
    const heard: string[] = [];
    const listen = (event: Event) => heard.push((event as CustomEvent<{ key: string }>).detail.key);
    window.addEventListener(REMOTE_DRAFT_EVENT, listen);
    useDraftStore.getState().setDraft('ch-1', 'never mind', { place: place('ch-1') });
    renderList();

    act(() => fireEvent.click(screen.getByTestId('draft-delete')));

    expect(screen.getByTestId('drafts-empty')).toBeInTheDocument();
    expect(useDraftStore.getState().entry('ch-1')).toMatchObject({ text: '', pending: true });
    expect(heard).toEqual(['ch-1']);
    window.removeEventListener(REMOTE_DRAFT_EVENT, listen);
  });
});

describe('DraftsList in Japanese', () => {
  beforeEach(async () => {
    vi.useRealTimers();
    useDraftStore.setState({ drafts: {} });
    await i18n.changeLanguage('ja');
  });

  afterEach(async () => {
    useDraftStore.setState({ drafts: {} });
    await i18n.changeLanguage('en');
  });

  it('says there are no drafts and that they wait on every device', () => {
    renderList();
    expect(screen.getByTestId('drafts-empty').textContent).toBe(
      '下書きはありません。メッセージを書きかけのまま送信せずにおくと、ここに残ります。サインインしているどの端末からでも見られます。',
    );
  });

  it('shows a draft as text, and says its files stayed behind', () => {
    useDraftStore
      .getState()
      .setDraft('ch-1/m-1', hostile, { place: place('ch-1', 'm-1'), filesLeftBehind: true });
    renderList();

    expect(screen.getByTestId('draft-content').textContent).toBe(hostile);
    expect(screen.getByTestId('draft-target')).toHaveTextContent('#design · スレッド');
    expect(screen.getByTestId('draft-files-left').textContent).toBe(
      '添付ファイルは、追加した端末にだけ残っています',
    );
    expect(screen.getByRole('button', { name: '下書きを削除' })).toBeInTheDocument();
    expect(document.querySelector('[onerror]')).toBeNull();
  });
});
