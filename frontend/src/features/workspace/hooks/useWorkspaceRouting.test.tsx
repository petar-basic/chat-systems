import { describe, it, expect, vi, beforeEach } from 'vitest';
import { useState } from 'react';
import { act, render, screen, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { MemoryRouter, Route, Routes, useLocation } from 'react-router';
import { useWorkspaceStore, type Workspace } from '@/stores/workspace';
import { placeIn, rememberPlace } from '../lastPlace';
import { useWorkspaceRouting } from './useWorkspaceRouting';

const workspace = { id: 'w1', name: 'Northwind', deleted_at: null } as unknown as Workspace;

interface ProbeProps {
  noWorkspaces?: boolean;
  workspaces?: Workspace[];
  conversations?: { id: string }[];
  refetchConversations?: () => Promise<unknown>;
}

function Probe({
  noWorkspaces = false,
  workspaces = [],
  conversations = [],
  refetchConversations = async () => undefined,
}: ProbeProps) {
  useWorkspaceRouting({
    workspaces,
    deletedWorkspaces: [],
    noWorkspaces,
    channels: [],
    conversations,
    conversationsLoaded: true,
    refetchConversations,
    markConversationReadServer: vi.fn(),
    panel: { close: vi.fn() } as never,
  });
  return <p data-qa="path">{useLocation().pathname}</p>;
}

function renderAt(path: string, probe: React.ReactElement) {
  render(
    <QueryClientProvider client={new QueryClient()}>
      <MemoryRouter initialEntries={[path]}>
        <Routes>
          <Route path="/app" element={probe} />
          <Route path="/app/:workspaceId" element={probe} />
          <Route path="/app/:workspaceId/:channelId" element={probe} />
          <Route path="/app/:workspaceId/c/:conversationId" element={probe} />
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe('useWorkspaceRouting', () => {
  it('leaves the url of a workspace once there are none left to be in', () => {
    renderAt('/app/w1/c1', <Probe noWorkspaces />);

    expect(screen.getByTestId('path')).toHaveTextContent(/^\/app$/);
  });

  it('keeps the url while the workspaces are still loading', () => {
    renderAt('/app/w1/c1', <Probe />);

    expect(screen.getByTestId('path')).toHaveTextContent('/app/w1/c1');
  });
});

describe('a conversation url', () => {
  beforeEach(() => {
    localStorage.clear();
    useWorkspaceStore.setState({ currentWorkspace: workspace, currentConversationId: null });
  });

  it('opens the workspace instead when it is not a conversation you are in, and is forgotten', async () => {
    rememberPlace('w1', '/app/w1/c/gone');
    const refetchConversations = vi.fn(async () => undefined);

    renderAt(
      '/app/w1/c/gone',
      <Probe workspaces={[workspace]} refetchConversations={refetchConversations} />,
    );

    await waitFor(() => expect(screen.getByTestId('path')).toHaveTextContent(/^\/app\/w1$/));
    expect(refetchConversations).toHaveBeenCalledTimes(1);
    expect(placeIn('w1')).toBe('/app/w1');
  });

  it('waits for the list to be fetched again before giving up on a conversation it does not have yet', async () => {
    function Fresh() {
      const [conversations, setConversations] = useState<{ id: string }[]>([]);
      return (
        <Probe
          workspaces={[workspace]}
          conversations={conversations}
          refetchConversations={async () => setConversations([{ id: 'new' }])}
        />
      );
    }

    renderAt('/app/w1/c/new', <Fresh />);
    await act(async () => {});

    expect(screen.getByTestId('path')).toHaveTextContent('/app/w1/c/new');
  });
});
