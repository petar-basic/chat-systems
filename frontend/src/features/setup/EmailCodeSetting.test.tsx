import { describe, it, expect, vi, afterEach } from 'vitest';
import i18n from '@/lib/i18n';
import { fireEvent, render, screen } from '@testing-library/react';
import { EmailCodeSetting } from './EmailCodeSetting';

const settings = vi.hoisted(() => ({
  data: { email_code_sign_in: true, email_configured: true },
  mutate: vi.fn(),
}));

vi.mock('@/features/instance', () => ({
  useInstanceSettings: () => ({ data: settings.data }),
  useUpdateInstanceSettings: () => ({ mutate: settings.mutate, isPending: false, isError: false }),
}));

describe('EmailCodeSetting', () => {
  afterEach(() => {
    settings.mutate.mockReset();
    settings.data = { email_code_sign_in: true, email_configured: true };
  });

  it('turns emailed codes off', () => {
    render(<EmailCodeSetting instanceUrl="https://chat.example.com" />);
    const toggle = screen.getByLabelText('Sign in with an emailed code');
    expect(toggle).toBeChecked();

    fireEvent.click(toggle);
    expect(settings.mutate).toHaveBeenCalledWith({ email_code_sign_in: false });
  });

  it('stays off while the instance cannot send email', () => {
    settings.data = { email_code_sign_in: true, email_configured: false };
    render(<EmailCodeSetting instanceUrl="https://chat.example.com" />);
    const toggle = screen.getByLabelText('Sign in with an emailed code');
    expect(toggle).not.toBeChecked();
    expect(toggle).toBeDisabled();
    expect(screen.getByText(/Needs SMTP/)).toBeInTheDocument();
  });
});

describe('EmailCodeSetting in Japanese', () => {
  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('says what the setting does, and what it needs', async () => {
    await i18n.changeLanguage('ja');
    const { unmount } = render(<EmailCodeSetting instanceUrl="https://chat.example.com" />);
    expect(screen.getByLabelText('メールで届いたコードでサインイン')).toBeChecked();
    expect(
      screen.getByText(
        'サインイン画面で、6桁のコードをメールで受け取れるようになります。パスワードがなくてもサインインできます。',
      ),
    ).toBeInTheDocument();
    unmount();

    settings.data = { email_code_sign_in: true, email_configured: false };
    render(<EmailCodeSetting instanceUrl="https://chat.example.com" />);
    expect(
      screen.getByText(
        'SMTP の設定が必要です。サーバーがメールを送れるようになるまで、サインイン画面には表示されません。',
      ),
    ).toBeInTheDocument();
  });
});
