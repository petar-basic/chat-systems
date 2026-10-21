import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { AppConnectPrompt } from './AppConnectPrompt';

const mutate = vi.fn();
vi.mock('@/hooks/queries/useApps', () => ({
  useAppActions: () => ({ connect: { mutate, isPending: false } }),
}));

describe('AppConnectPrompt', () => {
  beforeEach(() => {
    mutate.mockClear();
    localStorage.clear();
  });

  it('starts connecting the app and asks for this message to be refreshed after', () => {
    render(<AppConnectPrompt providers={['jira']} messageId="m-1" />);
    fireEvent.click(screen.getByText('Connect Jira to show a preview'));
    expect(mutate).toHaveBeenCalledWith({ provider: 'jira', refreshMessageId: 'm-1' });
  });

  it('stays dismissed', () => {
    const { unmount } = render(<AppConnectPrompt providers={['jira']} messageId="m-2" />);
    fireEvent.click(screen.getByLabelText('Dismiss'));
    expect(screen.queryByTestId('app-connect-prompt')).not.toBeInTheDocument();
    unmount();
    render(<AppConnectPrompt providers={['jira']} messageId="m-2" />);
    expect(screen.queryByTestId('app-connect-prompt')).not.toBeInTheDocument();
  });
});
