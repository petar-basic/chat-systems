import { describe, it, expect, vi, beforeEach } from 'vitest';
import type { ReactNode } from 'react';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { GeneralSection } from './GeneralSection';
import type { Workspace } from '@/stores/workspace';

const patch = vi.fn();

vi.mock('react-router', () => ({ useNavigate: () => vi.fn() }));
vi.mock('@/hooks/queries/useWorkspaces', () => ({
  useRestoreWorkspace: () => ({ mutateAsync: vi.fn(), isPending: false }),
}));
vi.mock('@/shared/hooks/useCurrentApi', () => ({
  getApiForInstance: () => ({
    typed: (call: (c: { PATCH: typeof patch }) => unknown) => call({ PATCH: patch }),
  }),
}));

const workspace = {
  id: 'ws-1',
  name: 'Dev Team',
  description: 'Where we build',
  instanceUrl: 'http://localhost:8080',
  settings: {},
} as unknown as Workspace;

function wrapper({ children }: { children: ReactNode }) {
  return <QueryClientProvider client={new QueryClient()}>{children}</QueryClientProvider>;
}

const bodyOf = (call: number) => patch.mock.calls[call][1].body;

describe('GeneralSection', () => {
  beforeEach(() => {
    patch.mockReset();
    patch.mockResolvedValue({ data: {} });
  });

  it('saves a renamed workspace when the field is left, and only the name', async () => {
    render(<GeneralSection workspace={workspace} onDeleted={vi.fn()} />, { wrapper });
    const field = screen.getByLabelText(/Workspace name/);
    fireEvent.change(field, { target: { value: '  Platform  ' } });
    fireEvent.blur(field);
    await waitFor(() => expect(patch).toHaveBeenCalledOnce());
    expect(bodyOf(0)).toEqual({ name: 'Platform' });
    expect(await screen.findByText('Saved')).toBeInTheDocument();
  });

  it('saves nothing when a field is left unchanged, and puts back an emptied name', () => {
    render(<GeneralSection workspace={workspace} onDeleted={vi.fn()} />, { wrapper });
    fireEvent.blur(screen.getByLabelText(/Description/));
    const field = screen.getByLabelText(/Workspace name/);
    fireEvent.change(field, { target: { value: '   ' } });
    fireEvent.blur(field);
    expect(patch).not.toHaveBeenCalled();
    expect(field).toHaveValue('Dev Team');
  });

  it('saves a switch the moment it changes', async () => {
    render(<GeneralSection workspace={workspace} onDeleted={vi.fn()} />, { wrapper });
    fireEvent.change(screen.getByTestId('workspace-workflow-builders'), { target: { value: 'admins' } });
    await waitFor(() => expect(patch).toHaveBeenCalledOnce());
    expect(bodyOf(0)).toEqual({ workflow_builders: 'admins' });
  });
});
