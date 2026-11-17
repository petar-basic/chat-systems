import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, within } from '@testing-library/react';
import i18n from '@/lib/i18n';
import { ScimSection } from './ScimSection';
import type { MintedScimToken, ScimToken } from '@/features/workspace/hooks/useScimTokens';

type Options = { onSuccess?: (token: MintedScimToken) => void };

const mintMutate = vi.fn();
const rotateMutate = vi.fn();
const revokeMutate = vi.fn();
let tokens: ScimToken[] = [];
const hostile = '<img src=x onerror="alert(1)"> {{name}}';

vi.mock('@/features/workspace/hooks/useScimTokens', () => ({
  useScimTokens: () => ({ data: tokens, isLoading: false, error: null }),
  useMintScimToken: () => ({ mutate: mintMutate, isPending: false, error: null }),
  useRotateScimToken: () => ({ mutate: rotateMutate, isPending: false, error: null }),
  useRevokeScimToken: () => ({ mutateAsync: revokeMutate, isPending: false, error: null }),
}));

const token = (over: Partial<ScimToken>): ScimToken => ({
  id: 'tok-1',
  description: 'Okta',
  created_by: 'admin-1',
  last_used_at: null,
  revoked_at: null,
  created_at: '2026-10-01T00:00:00Z',
  ...over,
});

describe('ScimSection', () => {
  beforeEach(() => {
    tokens = [];
    mintMutate
      .mockReset()
      .mockImplementation((_description: string, options?: Options) =>
        options?.onSuccess?.({ id: 'tok-2', description: 'Entra', token: 'secret-once' }),
      );
    rotateMutate
      .mockReset()
      .mockImplementation((_id: string, options?: Options) =>
        options?.onSuccess?.({ id: 'tok-3', description: 'Okta', token: 'secret-rotated' }),
      );
    revokeMutate.mockReset().mockResolvedValue({ status: 'revoked' });
  });

  it('shows a minted token once, and not after it is put away', () => {
    render(<ScimSection />);
    fireEvent.change(screen.getByLabelText(/Token name/), { target: { value: 'Entra' } });
    fireEvent.click(screen.getByTestId('scim-token-mint'));

    expect(mintMutate.mock.calls[0][0]).toBe('Entra');
    expect(screen.getByTestId('scim-token-value')).toHaveTextContent('secret-once');

    fireEvent.click(screen.getByTestId('scim-token-dismiss'));
    expect(screen.queryByTestId('scim-token-value')).not.toBeInTheDocument();
    expect(screen.queryByText('secret-once')).not.toBeInTheDocument();
  });

  it('rotating hands over the replacement the same way', () => {
    tokens = [token({})];
    render(<ScimSection />);
    fireEvent.click(screen.getByTestId('scim-token-rotate'));

    expect(rotateMutate.mock.calls[0][0]).toBe('tok-1');
    expect(screen.getByTestId('scim-token-value')).toHaveTextContent('secret-rotated');
  });

  it('a revoked token can be neither rotated nor revoked again', () => {
    tokens = [token({ revoked_at: '2026-10-20T00:00:00Z' }), token({ id: 'tok-4', description: null })];
    render(<ScimSection />);
    const [revoked, live] = screen.getAllByTestId('scim-token-row');

    expect(within(revoked).queryByTestId('scim-token-rotate')).not.toBeInTheDocument();
    expect(within(revoked).getByTestId('scim-token-state')).toHaveTextContent('Revoked');
    expect(within(live).getByText('Unnamed token')).toBeInTheDocument();
    expect(within(live).getByTestId('scim-token-state')).toHaveTextContent('never used');
  });

  it('revokes behind a confirmation', async () => {
    tokens = [token({})];
    render(<ScimSection />);
    fireEvent.click(screen.getByTestId('scim-token-revoke'));
    expect(revokeMutate).not.toHaveBeenCalled();

    fireEvent.click(await screen.findByTestId('scim-revoke-dialog-confirm'));
    expect(revokeMutate).toHaveBeenCalledWith('tok-1');
  });
});

describe('ScimSection in Japanese', () => {
  beforeEach(async () => {
    tokens = [];
    mintMutate
      .mockReset()
      .mockImplementation((_description: string, options?: Options) =>
        options?.onSuccess?.({ id: 'tok-2', description: null, token: 'secret-once' }),
      );
    await i18n.changeLanguage('ja');
  });

  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('explains tokens and hands a new one over once', () => {
    render(<ScimSection />);

    expect(screen.getByText('/api/scim/v2').tagName).toBe('CODE');
    expect(screen.getByText('/api/scim/v2').parentElement).toHaveTextContent(
      'Okta や Entra ID などの ID プロバイダーは、これらのトークンを使い、/api/scim/v2 を通じてこのサーバーのアカウントを作成・名前の変更・無効化します。',
    );
    expect(screen.getByText('トークンはまだありません。')).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText(/トークンの名前/), { target: { value: '' } });
    fireEvent.click(screen.getByRole('button', { name: 'トークンを作成' }));
    expect(
      screen.getByText('今すぐコピーしてください。二度と表示されず、あとから誰も読み出せません。'),
    ).toBeInTheDocument();
    expect(screen.getByText('SCIM トークン')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '安全な場所に保存しました' })).toBeInTheDocument();
  });

  it('names each token as text, and asks before revoking it', async () => {
    tokens = [
      token({ description: hostile, last_used_at: '2026-10-20T00:00:00Z' }),
      token({ id: 'tok-4', description: null }),
      token({ id: 'tok-5', revoked_at: '2026-10-21T00:00:00Z' }),
    ];
    render(<ScimSection />);
    const [used, unnamed, revoked] = screen.getAllByTestId('scim-token-row');

    expect(within(used).getByText(hostile)).toBeInTheDocument();
    expect(within(used).getByTestId('scim-token-state')).toHaveTextContent(/^最終使用: /);
    expect(within(unnamed).getByText('名前のないトークン')).toBeInTheDocument();
    expect(within(unnamed).getByTestId('scim-token-state')).toHaveTextContent(/ に作成、未使用$/);
    expect(within(revoked).getByTestId('scim-token-state')).toHaveTextContent(/ に無効化$/);

    expect(within(used).getByRole('button', { name: `${hostile} を再発行` })).toBeInTheDocument();
    fireEvent.click(within(used).getByRole('button', { name: `${hostile} を無効化` }));
    expect(await screen.findByText(`${hostile} を無効化しますか?`)).toBeInTheDocument();
    expect(
      screen.getByText(
        'このトークンを使っている ID プロバイダーは、すぐにプロビジョニングできなくなります。',
      ),
    ).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'トークンを無効化' })).toBeInTheDocument();
    expect(document.querySelector('[onerror]')).toBeNull();
  });
});
