import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { MemoryRouter, Route, Routes } from 'react-router';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import i18n from '@/lib/i18n';
import en from '@/locales/en/auth.json';
import ja from '@/locales/ja/auth.json';
import CompleteRegistrationPage from './CompleteRegistrationPage';

const { typed, store, registration, instanceInfo } = vi.hoisted(() => ({
  typed: vi.fn(),
  store: {
    instances: [] as { url: string; user: { email: string } }[],
    hydrated: true,
    removeInstance: () => {},
    setActiveInstance: () => {},
  },
  registration: { mutateAsync: () => Promise.resolve(), isPending: false, error: null as unknown },
  instanceInfo: { sso_enabled: false },
}));

vi.mock('@/lib/instances', () => ({
  instanceManager: { normalize: (url: string) => url, get: () => ({ api: { typed } }) },
}));
vi.mock('@/stores/instances', () => ({
  useInstanceStore: (select: (s: typeof store) => unknown) => select(store),
}));
vi.mock('@/features/instance/hooks/useAuth', () => ({ useCompleteRegistration: () => registration }));
vi.mock('@/features/instance/hooks/useInstanceInfo', () => ({
  useInstanceInfo: () => ({ data: instanceInfo }),
}));

const invite = (
  over: Partial<{ email: string; workspace_name: string; already_registered: boolean }> = {},
) => ({
  email: 'alice@example.com',
  workspace_name: 'Acme',
  workspace_id: 'ws-1',
  already_registered: false,
  ...over,
});

const renderPage = () =>
  render(
    <QueryClientProvider client={new QueryClient()}>
      <MemoryRouter initialEntries={['/invite/tok']}>
        <Routes>
          <Route path="/invite/:token" element={<CompleteRegistrationPage />} />
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>,
  );

const signedInAs = (email: string) => {
  store.instances = [{ url: window.location.origin, user: { email } }];
};

beforeEach(() => {
  typed.mockReset();
  typed.mockResolvedValue(invite());
  store.instances = [];
  registration.error = null;
  instanceInfo.sso_enabled = false;
});

describe('CompleteRegistrationPage in English', () => {
  it('keeps the registration wording', async () => {
    renderPage();
    expect(await screen.findByRole('heading', { name: 'Complete registration' })).toBeInTheDocument();
    expect(screen.getByText('Acme').parentElement?.textContent).toBe("You've been invited to Acme");
    expect(screen.getByPlaceholderText('John Doe')).toBeInTheDocument();
    expect(screen.getByPlaceholderText('Min 8 characters')).toBeInTheDocument();
    expect(screen.getByPlaceholderText('Confirm your password')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Create account' })).toBeInTheDocument();
  });

  it('keeps the wording for someone signed in as somebody else', async () => {
    typed.mockResolvedValue(invite({ already_registered: true }));
    signedInAs('bob@example.com');
    renderPage();
    expect(await screen.findByRole('heading', { name: 'Join Acme' })).toBeInTheDocument();
    expect(screen.getByText('bob@example.com').parentElement?.textContent).toBe(
      "You're signed in as bob@example.com, but this invite was sent to alice@example.com.",
    );
    expect(screen.getByRole('button', { name: 'Switch account' })).toBeInTheDocument();
  });
});

describe('CompleteRegistrationPage in Japanese', () => {
  beforeEach(async () => {
    await i18n.changeLanguage('ja');
  });

  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('asks a new person for a name and a password', async () => {
    renderPage();
    expect(await screen.findByRole('heading', { name: 'アカウントの登録' })).toBeInTheDocument();
    const workspace = screen.getByText('Acme');
    expect(workspace).toHaveClass('text-accent');
    expect(workspace.parentElement?.textContent).toBe('Acme に招待されています');
    expect(screen.getByLabelText('メールアドレス')).toHaveValue('alice@example.com');
    expect(screen.getByLabelText('表示名')).toHaveAttribute('placeholder', '例: 山田 太郎');
    expect(screen.getByLabelText('パスワード')).toHaveAttribute('placeholder', '8文字以上');
    expect(screen.getByLabelText('パスワード(確認)')).toHaveAttribute('placeholder', 'もう一度入力');
    expect(screen.getByRole('button', { name: 'アカウントを作成' })).toBeDisabled();
  });

  it('says when the passwords differ', async () => {
    renderPage();
    fireEvent.change(await screen.findByLabelText('パスワード'), { target: { value: 'correct horse' } });
    fireEvent.change(screen.getByLabelText('パスワード(確認)'), { target: { value: 'correct horsf' } });
    expect(screen.getByText('パスワードが一致しません')).toBeInTheDocument();
  });

  it('says registration failed when the server gives no reason', async () => {
    registration.error = 'nope';
    renderPage();
    expect(await screen.findByText('登録できませんでした')).toBeInTheDocument();
  });

  it('says when the invite link does not work', async () => {
    typed.mockRejectedValue(new Error('gone'));
    renderPage();
    expect(await screen.findByText('招待が無効です')).toBeInTheDocument();
    expect(screen.getByText('この招待リンクは無効か、期限が切れています。')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'サインイン画面へ' })).toBeInTheDocument();
  });

  it('lets the invited person join', async () => {
    typed.mockResolvedValueOnce(invite({ already_registered: true }));
    signedInAs('Alice@Example.com');
    renderPage();
    expect(await screen.findByRole('heading', { name: 'Acme に参加' })).toBeInTheDocument();
    expect(screen.getByText('alice@example.com').parentElement?.textContent).toBe(
      'alice@example.com 宛ての招待',
    );
    typed.mockImplementationOnce(() => Promise.reject('nope'));
    fireEvent.click(screen.getByRole('button', { name: 'Acme に参加' }));
    expect(await screen.findByText('このワークスペースに参加できませんでした。')).toBeInTheDocument();
  });

  it('asks someone signed in as somebody else to switch', async () => {
    typed.mockResolvedValue(invite({ already_registered: true }));
    signedInAs('bob@example.com');
    renderPage();
    const account = await screen.findByText('bob@example.com');
    expect(account).toHaveClass('font-medium');
    expect(account.parentElement?.textContent).toBe(
      'bob@example.com でサインインしていますが、この招待は alice@example.com 宛てです。',
    );
    expect(screen.getByRole('button', { name: 'アカウントを切り替える' })).toBeInTheDocument();
  });

  it('asks someone signed out to sign in, or use SSO', async () => {
    typed.mockResolvedValue(invite({ already_registered: true }));
    instanceInfo.sso_enabled = true;
    renderPage();
    expect(
      await screen.findByText(
        'このメールアドレスのアカウントは、このサーバーにすでにあります。サインインして招待を受けてください。',
      ),
    ).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'サインインして参加' })).toBeInTheDocument();
    expect(screen.getByText('または')).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'SSO で続ける' })).toBeInTheDocument();
  });

  it('names a workspace that has no name', async () => {
    typed.mockResolvedValue(invite({ already_registered: true, workspace_name: '' }));
    renderPage();
    expect(await screen.findByRole('heading', { name: 'ワークスペース に参加' })).toBeInTheDocument();
  });

  it('has Japanese for every string', () => {
    expect(Object.keys(ja.completeRegistration).sort()).toEqual(Object.keys(en.completeRegistration).sort());
    for (const [key, value] of Object.entries(ja.completeRegistration)) {
      expect(value, key).toMatch(/[぀-ヿ一-鿿]/);
    }
  });
});

describe.each([
  ['en', '<img src=x onerror="alert(1)"> {{name}}'],
  ['ja', '<img src=x onerror="alert(1)"> {{name}}'],
  ['en', '</workspace><strong>Locked</strong><workspace></email><strong>Locked</strong><email>'],
  ['ja', '</account><strong>Locked</strong><account></email><strong>Locked</strong><email>'],
])('with a hostile name (%s): %s', (language, hostile) => {
  beforeEach(async () => {
    await i18n.changeLanguage(language);
  });

  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  const safe = () => {
    expect(document.querySelector('[onerror]')).toBeNull();
    expect(document.querySelector('strong')).toBeNull();
  };

  it('shows a workspace name as text when registering', async () => {
    typed.mockResolvedValue(invite({ workspace_name: hostile }));
    renderPage();
    const name = await screen.findByText(hostile);
    expect(name.textContent).toBe(hostile);
    expect(name).toHaveClass('text-accent');
    safe();
  });

  it('shows a workspace name and email as text when joining', async () => {
    typed.mockResolvedValue(invite({ already_registered: true, workspace_name: hostile, email: hostile }));
    signedInAs(hostile);
    renderPage();
    const join = language === 'ja' ? `${hostile} に参加` : `Join ${hostile}`;
    expect((await screen.findByRole('heading')).textContent).toBe(join);
    expect(screen.getByRole('button', { name: join }).textContent).toBe(join);
    expect(screen.getByText(hostile).textContent).toBe(hostile);
    safe();
  });

  it('shows both emails as text when signed in as somebody else', async () => {
    typed.mockResolvedValue(invite({ already_registered: true, email: hostile }));
    signedInAs(`${hostile}x`);
    renderPage();
    const account = await screen.findByText(`${hostile}x`);
    expect(account.textContent).toBe(`${hostile}x`);
    expect(screen.getAllByText(hostile).map((e) => e.textContent)).toEqual([hostile, hostile]);
    safe();
  });
});
