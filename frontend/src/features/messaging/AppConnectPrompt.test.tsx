import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { AppConnectPrompt } from './AppConnectPrompt';

const connect = vi.fn();
const prompts = vi.fn();
let apps: { provider: string; prompts_off: boolean; connected_account: string | null }[] = [];
vi.mock('@/hooks/queries/useApps', () => ({
  useAppActions: () => ({
    connect: { mutate: connect, isPending: false },
    prompts: { mutate: prompts, isPending: false },
  }),
  useApps: () => ({ data: { data: apps } }),
}));

describe('AppConnectPrompt', () => {
  beforeEach(() => {
    connect.mockClear();
    prompts.mockClear();
    apps = [];
    localStorage.clear();
  });

  it('starts connecting the app and asks for this message to be refreshed after', () => {
    render(<AppConnectPrompt providers={['jira']} messageId="m-1" />);
    fireEvent.click(screen.getByText('Connect Jira'));
    expect(connect).toHaveBeenCalledWith({ provider: 'jira', refreshMessageId: 'm-1' });
  });

  it('stays dismissed after Not now', () => {
    const { unmount } = render(<AppConnectPrompt providers={['jira']} messageId="m-2" />);
    fireEvent.click(screen.getByText('Not now'));
    expect(screen.queryByTestId('app-connect-prompt')).not.toBeInTheDocument();
    unmount();
    render(<AppConnectPrompt providers={['jira']} messageId="m-2" />);
    expect(screen.queryByTestId('app-connect-prompt')).not.toBeInTheDocument();
  });

  it('turns the app prompts off for good', () => {
    render(<AppConnectPrompt providers={['jira']} messageId="m-3" />);
    fireEvent.click(screen.getByText('Never ask me again'));
    expect(prompts).toHaveBeenCalledWith({ provider: 'jira', off: true });
  });

  it('asks about nothing already connected or turned off', () => {
    apps = [
      { provider: 'jira', prompts_off: true, connected_account: null },
      { provider: 'github', prompts_off: false, connected_account: 'ana' },
    ];
    render(<AppConnectPrompt providers={['jira', 'github']} messageId="m-4" />);
    expect(screen.queryByTestId('app-connect-prompt')).not.toBeInTheDocument();
  });
});
