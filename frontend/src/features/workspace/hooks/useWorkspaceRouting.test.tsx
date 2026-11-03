import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { MemoryRouter, Route, Routes, useLocation } from 'react-router';
import { useWorkspaceRouting } from './useWorkspaceRouting';

function Probe({ noWorkspaces }: { noWorkspaces: boolean }) {
  useWorkspaceRouting({
    workspaces: [],
    deletedWorkspaces: [],
    noWorkspaces,
    channels: [],
    markConversationReadServer: vi.fn(),
    panel: { close: vi.fn() } as never,
  });
  return <p data-qa="path">{useLocation().pathname}</p>;
}

function renderAt(path: string, noWorkspaces: boolean) {
  render(
    <QueryClientProvider client={new QueryClient()}>
      <MemoryRouter initialEntries={[path]}>
        <Routes>
          <Route path="/app" element={<Probe noWorkspaces={noWorkspaces} />} />
          <Route path="/app/:workspaceId/:channelId" element={<Probe noWorkspaces={noWorkspaces} />} />
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe('useWorkspaceRouting', () => {
  it('leaves the url of a workspace once there are none left to be in', () => {
    renderAt('/app/w1/c1', true);

    expect(screen.getByTestId('path')).toHaveTextContent(/^\/app$/);
  });

  it('keeps the url while the workspaces are still loading', () => {
    renderAt('/app/w1/c1', false);

    expect(screen.getByTestId('path')).toHaveTextContent('/app/w1/c1');
  });
});
