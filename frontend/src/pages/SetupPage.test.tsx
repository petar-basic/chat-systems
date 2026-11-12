import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import i18n from '@/lib/i18n';
import SetupPage from './SetupPage';

const HOSTILE = '<img src=x onerror="alert(1)"> {{name}}';

const info = vi.hoisted(() => ({
  current: {} as Record<string, unknown>,
}));

vi.mock('react-router', () => ({ useNavigate: () => vi.fn() }));
vi.mock('@/features/instance/hooks/useInstanceInfo', () => ({
  useInstanceInfo: () => info.current,
}));
vi.mock('@/features/setup/SetupForm', () => ({ SetupForm: () => <p>setup form</p> }));
vi.mock('@/features/setup/TestEmailCheck', () => ({ TestEmailCheck: () => null }));

describe('SetupPage in Japanese', () => {
  beforeEach(async () => {
    await i18n.changeLanguage('ja');
  });

  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('names the instance as plain text', () => {
    info.current = { isPending: false, isError: false, data: { name: HOSTILE, setup_required: true } };
    render(<SetupPage />);
    expect(screen.getByRole('heading', { level: 1 }).textContent).toBe(`${HOSTILE} のセットアップ`);
    expect(screen.getByText('最初の管理者アカウントとワークスペースを作成します。')).toBeInTheDocument();
    expect(screen.getByText('setup form')).toBeInTheDocument();
    expect(document.querySelector('[onerror]')).toBeNull();
  });

  it('says when the instance is set up already', () => {
    info.current = { isPending: false, isError: false, data: { name: 'Chat', setup_required: false } };
    render(<SetupPage />);
    expect(screen.getByTestId('setup-not-needed')).toHaveTextContent('サーバーはセットアップ済みです。');
    expect(screen.getByRole('button', { name: 'サインイン' })).toBeInTheDocument();
  });

  it('says when the instance cannot be reached', () => {
    info.current = { isPending: false, isError: true, error: {}, data: undefined };
    render(<SetupPage />);
    expect(screen.getByText('サーバーに接続できませんでした')).toBeInTheDocument();
  });
});
