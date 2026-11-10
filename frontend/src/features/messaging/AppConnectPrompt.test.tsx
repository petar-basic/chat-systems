import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import i18n from '@/lib/i18n';
import { AppConnectPrompt } from './AppConnectPrompt';

const connect = vi.fn();
const prompts = vi.fn();
let apps: { provider: string; prompts_off: boolean; connected_account: string | null }[] = [];
vi.mock('@/features/workspace/hooks/useApps', () => ({
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

describe('AppConnectPrompt in Japanese', () => {
  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('asks to connect the app by name, with the name as text', async () => {
    const provider = '<img src=x onerror="alert(1)"> {{name}}';
    apps = [];
    await i18n.changeLanguage('ja');
    render(<AppConnectPrompt providers={[provider]} messageId="m-9" />);
    expect(screen.getByText('· あなたにだけ表示されています')).toBeInTheDocument();
    expect(
      screen.getByText(`このリンクのプレビューを表示するには、${provider} のアカウントを接続してください。`),
    ).toBeInTheDocument();
    expect(screen.getByRole('button', { name: `${provider} を接続` })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '今はしない' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '今後は表示しない' })).toBeInTheDocument();
    expect(document.querySelector('[onerror]')).toBeNull();
  });
});
