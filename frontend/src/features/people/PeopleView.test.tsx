import { describe, it, expect, vi, beforeEach } from 'vitest';
import '@/lib/i18n';
import { fireEvent, render, screen } from '@testing-library/react';
import { MemoryRouter, Route, Routes, useLocation } from 'react-router';
import type { WorkspaceMember } from '@/stores/workspace';
import type { ProfileField } from './hooks/useProfileFields';
import { PeopleView } from './PeopleView';

const directory = vi.hoisted(() => vi.fn());

const fields: ProfileField[] = [
  {
    id: 'dept',
    workspace_id: 'ws',
    label: 'Department',
    kind: 'options',
    options: [
      { id: 'fin', label: 'Finance' },
      { id: 'sup', label: 'Support' },
    ],
    admins_only: false,
    on_card: true,
    position: 0,
    value_count: 2,
  },
  {
    id: 'office',
    workspace_id: 'ws',
    label: 'Office',
    kind: 'text',
    options: [],
    admins_only: false,
    on_card: false,
    position: 1,
    value_count: 1,
  },
];

const member = (
  user_id: string,
  display_name: string,
  profile_fields: Record<string, string>,
): WorkspaceMember => ({
  workspace_id: 'ws',
  user_id,
  role: 'member',
  joined_at: '2026-01-01T00:00:00Z',
  email: `${user_id}@example.com`,
  display_name,
  is_bot: false,
  profile_fields,
});

const ana = member('ana', 'Ana Admir', { dept: 'fin' });

vi.mock('./hooks/useProfileFields', () => ({
  useProfileFields: () => ({ fields, isSuccess: true }),
  useDirectory: directory,
}));

function Where() {
  const location = useLocation();
  return <output data-qa="where">{location.search}</output>;
}

function renderAt(url: string) {
  render(
    <MemoryRouter initialEntries={[url]}>
      <Routes>
        <Route
          path="/app/:workspaceId/people"
          element={
            <>
              <PeopleView workspaceId="ws" members={[ana]} />
              <Where />
            </>
          }
        />
      </Routes>
    </MemoryRouter>,
  );
}

describe('PeopleView', () => {
  beforeEach(() => {
    directory.mockReset();
    directory.mockReturnValue({ data: [ana], isSuccess: true, isError: false });
  });

  it('reads its filters from the address, so a filtered view can be shared', () => {
    renderAt('/app/ws/people?filter=dept:fin,office:x');
    expect(directory).toHaveBeenLastCalledWith('ws', '', 'dept:fin', undefined, true);
    expect(screen.getByTestId('people-filter-dept')).toHaveValue('fin');
    expect(screen.queryByTestId('people-filter-office')).not.toBeInTheDocument();
    expect(screen.getByTestId('people-row')).toHaveTextContent('Ana Admir');
    expect(screen.getByTestId('people-row')).toHaveTextContent('Finance');
  });

  it('writes a picked filter back into the address', () => {
    renderAt('/app/ws/people');
    fireEvent.change(screen.getByTestId('people-filter-dept'), { target: { value: 'sup' } });
    expect(screen.getByTestId('where')).toHaveTextContent('?filter=dept%3Asup');
    expect(directory).toHaveBeenLastCalledWith('ws', '', 'dept:sup', undefined, true);
  });

  it('offers to clear what found nobody', () => {
    directory.mockReturnValue({ data: [], isSuccess: true, isError: false });
    renderAt('/app/ws/people?filter=dept:sup');
    fireEvent.click(screen.getByTestId('people-clear'));
    expect(screen.getByTestId('where')).toBeEmptyDOMElement();
  });
});
