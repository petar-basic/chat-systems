import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, within } from '@testing-library/react';
import { ScimSection } from './ScimSection';
import type { MintedScimToken, ScimToken } from '@/features/workspace/hooks/useScimTokens';

type Options = { onSuccess?: (token: MintedScimToken) => void };

const mintMutate = vi.fn();
const rotateMutate = vi.fn();
const revokeMutate = vi.fn();
let tokens: ScimToken[] = [];

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
