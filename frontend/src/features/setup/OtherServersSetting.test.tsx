import { describe, it, expect, vi, afterEach } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import { OtherServersSetting } from './OtherServersSetting';

const settings = vi.hoisted(() => ({
  data: { other_servers_allowed: true },
  mutate: vi.fn(),
  isError: false,
  error: null as Error | null,
}));

vi.mock('@/features/instance', () => ({
  useInstanceSettings: () => ({ data: settings.data }),
  useUpdateInstanceSettings: () => ({
    mutate: settings.mutate,
    isPending: false,
    isError: settings.isError,
    error: settings.error,
  }),
}));

describe('OtherServersSetting', () => {
  afterEach(() => {
    settings.mutate.mockReset();
    settings.data = { other_servers_allowed: true };
    settings.isError = false;
    settings.error = null;
  });

  it('keeps everyone to this server, and changes nothing else', () => {
    render(<OtherServersSetting instanceUrl="https://chat.example.com" />);
    const toggle = screen.getByLabelText('Let people connect to other servers');
    expect(toggle).toBeChecked();
    expect(screen.getByText(/asks for a server URL/)).toBeInTheDocument();

    fireEvent.click(toggle);
    expect(settings.mutate).toHaveBeenCalledWith({ other_servers_allowed: false });
  });

  it('says what goes away while it is off, and lets other servers back in', () => {
    settings.data = { other_servers_allowed: false };
    render(<OtherServersSetting instanceUrl="https://chat.example.com" />);
    const toggle = screen.getByLabelText('Let people connect to other servers');
    expect(toggle).not.toBeChecked();
    expect(screen.getByText(/hides Add Server/)).toBeInTheDocument();

    fireEvent.click(toggle);
    expect(settings.mutate).toHaveBeenCalledWith({ other_servers_allowed: true });
  });

  it('says so when the setting could not be saved', () => {
    settings.isError = true;
    settings.error = new Error('Not allowed');
    render(<OtherServersSetting instanceUrl="https://chat.example.com" />);
    expect(screen.getByRole('alert')).toHaveTextContent('Not allowed');
  });
});
