import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, within } from '@testing-library/react';
import { ExportsSection } from './ExportsSection';
import type { ExportDetail } from '@/hooks/queries/useExports';
import type { WorkspaceMember } from '@/stores/workspace';

const startMutate = vi.fn();
let rows: ExportDetail[] = [];

vi.mock('@/hooks/queries/useExports', () => ({
  useExports: () => ({ data: rows, isLoading: false, error: null }),
  useStartWorkspaceExport: () => ({ mutate: startMutate, isPending: false, isError: false }),
}));

const row = (over: Partial<ExportDetail['export']>, download_url: string | null = null): ExportDetail => ({
  export: {
    id: 'exp-1',
    scope: 'workspace',
    workspace_id: 'ws-1',
    subject_user_id: null,
    requested_by: 'owner-1',
    include_dms: false,
    since: null,
    until: null,
    status: 'complete',
    storage_key: 'exports/exp-1.tar',
    manifest: null,
    error: null,
    token_expires_at: '2026-10-27T00:00:00Z',
    created_at: '2026-10-26T10:00:00Z',
    completed_at: '2026-10-26T10:01:00Z',
    ...over,
  },
  download_url,
});

const members = [
  { user_id: 'bob-1', display_name: 'Bob Smith', email: 'bob@dev.local' },
] as WorkspaceMember[];

describe('ExportsSection', () => {
  beforeEach(() => {
    rows = [];
    startMutate.mockReset();
  });

  it('lists past exports with their status and a download for the requester', () => {
    rows = [
      row({ id: 'exp-2', status: 'running', created_at: '2026-10-26T11:00:00Z' }),
      row({}, 'http://localhost:3000/api/exports/download/tok'),
      row({ id: 'exp-3', scope: 'user', workspace_id: null, subject_user_id: 'bob-1' }),
    ];
    render(<ExportsSection workspaceId="ws-1" isOwner members={members} />);

    const [running, ready, person] = screen.getAllByTestId('export-row');
    expect(within(running).getByTestId('export-status')).toHaveTextContent('running');
    expect(within(running).queryByTestId('export-download')).not.toBeInTheDocument();

    const link = within(ready).getByTestId('export-download');
    expect(link).toHaveAttribute('href', 'http://localhost:3000/api/exports/download/tok');
    fireEvent.click(link);
    expect(within(ready).queryByTestId('export-download')).not.toBeInTheDocument();
    expect(within(ready).getByTestId('export-link-gone')).toHaveTextContent('Downloaded');

    expect(within(person).getByTestId('export-subject')).toHaveTextContent('Bob Smith');
    expect(within(person).getByTestId('export-link-gone')).toHaveTextContent('Link used or expired');
  });

  it('starts a workspace export, with direct messages only when asked', () => {
    render(<ExportsSection workspaceId="ws-1" isOwner members={members} />);
    fireEvent.click(screen.getByTestId('export-start'));
    expect(startMutate).toHaveBeenLastCalledWith(false);

    fireEvent.click(screen.getByLabelText('Include direct messages'));
    fireEvent.click(screen.getByTestId('export-start'));
    expect(startMutate).toHaveBeenLastCalledWith(true);
  });

  it('leaves starting one to the owner', () => {
    render(<ExportsSection workspaceId="ws-1" isOwner={false} members={members} />);
    expect(screen.queryByTestId('export-start')).not.toBeInTheDocument();
    expect(screen.getByText('No exports yet.')).toBeInTheDocument();
  });
});
