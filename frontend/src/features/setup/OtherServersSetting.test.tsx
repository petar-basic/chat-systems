import { describe, it, expect, vi, afterEach } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import i18n from '@/lib/i18n';
import { OtherServersSetting } from './OtherServersSetting';

const hostile = '<img src=x onerror="alert(1)"> {{name}}';

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

  describe('in Japanese', () => {
    afterEach(async () => {
      await i18n.changeLanguage('en');
    });

    it('names the switch and says what it allows', async () => {
      await i18n.changeLanguage('ja');
      render(<OtherServersSetting instanceUrl="https://chat.example.com" />);
      expect(screen.getByLabelText('ほかのサーバーへの接続を許可する')).toBeChecked();
      expect(
        screen.getByText(
          'サインイン画面でサーバーの URL を入力でき、左端に「サーバーを追加」が表示されます。誰でも、同じウィンドウでほかの Chat Systems のサーバーを使えます。',
        ),
      ).toBeInTheDocument();
    });

    it('says what goes away while it is off', async () => {
      settings.data = { other_servers_allowed: false };
      await i18n.changeLanguage('ja');
      render(<OtherServersSetting instanceUrl="https://chat.example.com" />);
      expect(screen.getByLabelText('ほかのサーバーへの接続を許可する')).not.toBeChecked();
      expect(
        screen.getByText(
          'このサーバーだけを使います。サインイン画面にも左端にもほかのサーバーは表示されず、以前に追加したサーバーは次にアプリを読み込んだときに外れます。デスクトップアプリでも、このサーバーが含まれているあいだは「Add Server…」が表示されません。',
        ),
      ).toBeInTheDocument();
    });

    it('has Japanese for every one', () => {
      const english = Object.keys(i18n.getResource('en', 'admin', 'otherServers'));
      const japanese = Object.keys(i18n.getResource('ja', 'admin', 'otherServers'));
      expect(english.length).toBeGreaterThan(0);
      expect(japanese).toEqual(english);
    });
  });

  it.each([true, false])('shows the text as text when it is %s', (on) => {
    const keys = ['otherServers.label', 'otherServers.on', 'otherServers.off'] as const;
    const saved = keys.map((k) => i18n.t(k, { ns: 'admin' }));
    keys.forEach((k) => i18n.addResource('en', 'admin', k, hostile));
    try {
      settings.data = { other_servers_allowed: on };
      render(<OtherServersSetting instanceUrl="https://chat.example.com" />);
      expect(screen.getByLabelText(hostile)).toBeInTheDocument();
      expect(screen.getAllByText(hostile)).toHaveLength(2);
      expect(document.querySelector('[onerror]')).toBeNull();
    } finally {
      keys.forEach((k, i) => i18n.addResource('en', 'admin', k, saved[i]));
    }
  });
});
