import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import i18n from '@/lib/i18n';
import { render, screen, fireEvent } from '@testing-library/react';
import { SetupForm } from './SetupForm';

const mutate = vi.fn();
let mutation: { isPending: boolean; isError: boolean; error: unknown } = {
  isPending: false,
  isError: false,
  error: null,
};

vi.mock('@/features/instance/hooks/useAuth', () => ({
  useCompleteSetup: () => ({ mutate, ...mutation }),
}));

function fill(values: Record<string, string>) {
  for (const [qa, value] of Object.entries(values)) {
    fireEvent.change(screen.getByTestId(qa), { target: { value } });
  }
}

const filled = {
  'setup-token': '  tok-123  ',
  'setup-name': 'Ada',
  'setup-email': 'ada@acme.test',
  'setup-password': 'a-long-password',
  'setup-workspace': 'Acme',
};

describe('SetupForm', () => {
  beforeEach(() => {
    mutate.mockReset();
    mutation = { isPending: false, isError: false, error: null };
  });

  it('sends the token, the first admin and the workspace together', () => {
    const onDone = vi.fn();
    render(<SetupForm onDone={onDone} />);
    fill(filled);
    fireEvent.click(screen.getByTestId('setup-submit'));

    expect(mutate).toHaveBeenCalledWith(
      {
        token: '  tok-123  ',
        displayName: 'Ada',
        email: 'ada@acme.test',
        password: 'a-long-password',
        workspaceName: 'Acme',
      },
      expect.anything(),
    );

    const [, options] = mutate.mock.calls[0] as [
      unknown,
      { onSuccess: (res: { workspace_id: string }) => void },
    ];
    options.onSuccess({ workspace_id: 'ws-1' });
    expect(onDone).toHaveBeenCalledWith('ws-1');
  });

  it('holds back a password that is too short', () => {
    render(<SetupForm onDone={vi.fn()} />);
    fill({ ...filled, 'setup-password': 'short' });
    fireEvent.click(screen.getByTestId('setup-submit'));

    expect(mutate).not.toHaveBeenCalled();
    expect(screen.getByText('At least 8 characters')).toBeInTheDocument();
  });

  it('says why the server refused', () => {
    mutation = {
      isPending: false,
      isError: true,
      error: new Error('That setup token is not right. Copy it again from the API log'),
    };
    render(<SetupForm onDone={vi.fn()} />);

    expect(screen.getByTestId('setup-error')).toHaveTextContent('That setup token is not right');
  });
});

describe('SetupForm in Japanese', () => {
  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('labels every field', async () => {
    await i18n.changeLanguage('ja');
    render(<SetupForm onDone={vi.fn()} />);
    for (const label of [
      'セットアップトークン',
      '名前',
      'メールアドレス',
      'パスワード',
      'ワークスペース名',
    ]) {
      expect(screen.getByLabelText(label, { exact: false })).toBeInTheDocument();
    }
    expect(
      screen.getByText('セットアップが終わるまで、API が起動するたびにログに出力します。'),
    ).toBeInTheDocument();
    expect(screen.getByTestId('setup-workspace')).toHaveAttribute('placeholder', '例: 営業部');
    fireEvent.change(screen.getByTestId('setup-password'), { target: { value: 'short' } });
    expect(screen.getByText('8文字以上にしてください')).toBeInTheDocument();
    expect(screen.getByTestId('setup-submit')).toHaveTextContent('サーバーをセットアップ');
  });
});
