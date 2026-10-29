import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { ConnectedAppCard } from './ConnectedAppCard';
import type { AppStatus } from '@/features/workspace/hooks/useApps';

const configureMutate = vi.fn();

vi.mock('@/features/workspace/hooks/useApps', () => ({
  useAppActions: () => ({
    configure: { mutateAsync: configureMutate, isPending: false },
    remove: { mutate: vi.fn(), isPending: false },
    connect: { mutate: vi.fn(), isPending: false },
    disconnect: { mutate: vi.fn(), isPending: false },
    prompts: { mutate: vi.fn(), isPending: false },
  }),
}));

const PERMISSIONS = [
  { key: 'jira.0', text: 'See the issues and projects each person who connects can see' },
  { key: 'jira.1', text: 'Stay connected until that person disconnects' },
];

const status = (over: Partial<AppStatus> = {}): AppStatus => ({
  provider: 'jira',
  name: 'Jira Cloud',
  summary: 'Issues',
  configured: false,
  enabled: false,
  prompts_off: false,
  client_id: null,
  connected_account: null,
  console_url: 'https://developer.atlassian.com/console/myapps/',
  redirect_url: 'https://chat.test/api/apps/jira/callback',
  scopes: ['read:jira-work'],
  setup_steps: ['Make an app'],
  ...over,
});

function renderCard(over: Partial<AppStatus> = {}) {
  render(<ConnectedAppCard workspaceId="ws-1" app={status(over)} isAdmin permissions={PERMISSIONS} />);
}

describe('ConnectedAppCard', () => {
  beforeEach(() => {
    configureMutate.mockReset().mockResolvedValue({});
  });

  it('installs only after an admin agrees to what it can see', async () => {
    renderCard();
    fireEvent.change(screen.getByLabelText(/Client ID/), { target: { value: 'client-1' } });
    fireEvent.change(screen.getByLabelText(/Secret/), { target: { value: 'shh' } });
    fireEvent.submit(screen.getByTestId('app-setup-form'));

    expect(configureMutate).not.toHaveBeenCalled();
    const consent = await screen.findByTestId('app-consent-permissions');
    expect(consent).toHaveTextContent('See the issues and projects');
    expect(consent).toHaveTextContent('Stay connected until that person disconnects');

    fireEvent.click(screen.getByTestId('app-consent-confirm'));
    await waitFor(() =>
      expect(configureMutate.mock.calls[0][0]).toEqual({
        provider: 'jira',
        clientId: 'client-1',
        clientSecret: 'shh',
        enabled: true,
      }),
    );
  });

  it('saves a change to an installed app without asking again', () => {
    renderCard({ configured: true, enabled: true, client_id: 'client-1' });
    fireEvent.click(screen.getByText('Edit setup'));
    fireEvent.submit(screen.getByTestId('app-setup-form'));

    expect(screen.queryByTestId('app-consent-permissions')).not.toBeInTheDocument();
    expect(configureMutate).toHaveBeenCalled();
  });
});
