import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import i18n from '@/lib/i18n';
import { render, screen, fireEvent } from '@testing-library/react';
import { TestEmailCheck } from './TestEmailCheck';
import type { EmailTest } from '@/features/instance/hooks/useAdmin';

const HOSTILE = '<img src=x onerror="alert(1)"> {{name}}';

const mutate = vi.fn();
let mutation: { isPending: boolean; isError: boolean; error: unknown; data: EmailTest | undefined } = {
  isPending: false,
  isError: false,
  error: null,
  data: undefined,
};

vi.mock('@/features/instance/hooks/useAdmin', () => ({
  useSendTestEmail: () => ({ mutate, ...mutation }),
}));

describe('TestEmailCheck', () => {
  beforeEach(() => {
    mutate.mockReset();
    mutation = { isPending: false, isError: false, error: null, data: undefined };
  });

  it('sends on request', () => {
    render(<TestEmailCheck instanceUrl="http://chat.test" />);
    fireEvent.click(screen.getByTestId('test-email-send'));
    expect(mutate).toHaveBeenCalled();
  });

  it('names the address it went to', () => {
    mutation.data = { sent_to: 'ada@acme.test', error: null };
    render(<TestEmailCheck instanceUrl="http://chat.test" />);
    expect(screen.getByTestId('test-email-sent')).toHaveTextContent('Sent to ada@acme.test');
  });

  it('shows the SMTP problem in words', () => {
    mutation.data = {
      sent_to: 'ada@acme.test',
      error: 'Nothing accepted the connection at smtp.acme.test:587. Check SMTP_HOST and SMTP_PORT',
    };
    render(<TestEmailCheck instanceUrl="http://chat.test" />);
    expect(screen.getByTestId('test-email-failed')).toHaveTextContent('Nothing accepted the connection');
    expect(screen.queryByTestId('test-email-sent')).toBeNull();
  });
});

describe('TestEmailCheck in Japanese', () => {
  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('explains the check and names the address as plain text', async () => {
    await i18n.changeLanguage('ja');
    mutation = { isPending: false, isError: false, error: null, data: { sent_to: HOSTILE, error: null } };
    render(<TestEmailCheck instanceUrl="http://chat.test" />);
    expect(
      screen.getByText(
        '招待やパスワード再設定のリンクはメールで届きます。SMTP の設定が正しいか、自分宛てにテストメールを送って確かめてください。',
      ),
    ).toBeInTheDocument();
    expect(screen.getByTestId('test-email-send')).toHaveTextContent('テストメールを送信');
    expect(screen.getByTestId('test-email-sent').textContent).toBe(
      `${HOSTILE} に送信しました。受信トレイと迷惑メールフォルダを確認してください。`,
    );
    expect(document.querySelector('[onerror]')).toBeNull();
  });
});
