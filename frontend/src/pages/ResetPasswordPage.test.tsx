import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { MemoryRouter } from 'react-router';
import i18n from '@/lib/i18n';
import ResetPasswordPage from './ResetPasswordPage';

const typed = vi.fn();

vi.mock('@/lib/api', () => ({
  api: { typed: (...args: unknown[]) => typed(...args) },
}));

function renderAt(path: string) {
  return render(
    <MemoryRouter initialEntries={[path]}>
      <ResetPasswordPage />
    </MemoryRouter>,
  );
}

beforeEach(async () => {
  typed.mockReset();
  typed.mockResolvedValue({});
  await i18n.changeLanguage('ja');
});

afterEach(async () => {
  await i18n.changeLanguage('en');
});

describe('ResetPasswordPage in Japanese', () => {
  it('asks for the address and says where the link went', async () => {
    renderAt('/forgot-password');
    expect(screen.getByRole('heading', { name: 'パスワードの再設定' })).toBeInTheDocument();

    fireEvent.change(screen.getByTestId('forgot-password-email'), { target: { value: 'alice@example.com' } });
    fireEvent.click(screen.getByRole('button', { name: '再設定用のリンクを送信' }));

    const sent = await screen.findByTestId('forgot-password-sent');
    expect(sent).toHaveTextContent(
      'alice@example.com が Chat Systems に登録されているメールアドレスであれば',
    );
    expect(screen.getByText('alice@example.com')).toHaveClass('font-medium');
  });

  it('explains a mismatch before it reaches the server', () => {
    renderAt('/reset-password?token=abc');

    fireEvent.change(screen.getByTestId('reset-password-new'), { target: { value: 'correct horse' } });
    fireEvent.change(screen.getByTestId('reset-password-confirm'), { target: { value: 'battery staple' } });
    fireEvent.click(screen.getByRole('button', { name: 'パスワードを再設定' }));

    expect(screen.getByRole('alert')).toHaveTextContent('パスワードが一致しません');
    expect(typed).not.toHaveBeenCalled();
  });
});
