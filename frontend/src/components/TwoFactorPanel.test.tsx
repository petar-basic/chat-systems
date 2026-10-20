import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import TwoFactorPanel from './TwoFactorPanel';

const get = vi.fn();
const post = vi.fn();

vi.mock('../lib/api', async () => {
  const { typedApiMock } = await import('@/test/typedApiMock');
  return {
    api: typedApiMock({ get: (...args) => get(...args), post: (...args) => post(...args) }),
  };
});

vi.mock('@/shared/components/Toast', () => ({
  toast: { success: vi.fn(), error: vi.fn() },
}));

function renderPanel() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <QueryClientProvider client={client}>
      <TwoFactorPanel />
    </QueryClientProvider>,
  );
}

describe('TwoFactorPanel', () => {
  beforeEach(() => {
    get.mockReset();
    post.mockReset();
  });

  it('does not treat an unfinished enrolment as protection', async () => {
    get.mockResolvedValue({ enrolled: false, recovery_codes_remaining: 0, required: false });
    post.mockResolvedValue({ secret: 'JBSWY3DPEHPK3PXP', provisioning_uri: 'otpauth://totp/x' });
    renderPanel();

    fireEvent.click(await screen.findByTestId('totp-start'));

    expect(await screen.findByText('JBSWY3DPEHPK3PXP')).toBeInTheDocument();
    expect(screen.getByTestId('totp-confirm')).toBeDisabled();
    expect(post).toHaveBeenCalledWith('/auth/totp/enrol', undefined);
    expect(post).not.toHaveBeenCalledWith('/auth/totp/confirm', expect.anything());
  });

  it('shows the recovery codes once the code is confirmed', async () => {
    get.mockResolvedValue({ enrolled: false, recovery_codes_remaining: 0, required: false });
    post.mockImplementation(async (path: string) =>
      path === '/auth/totp/enrol'
        ? { secret: 'SECRET', provisioning_uri: 'otpauth://totp/x' }
        : { recovery_codes: ['aaaaa-bbbbb', 'ccccc-ddddd'] },
    );
    renderPanel();

    fireEvent.click(await screen.findByTestId('totp-start'));
    fireEvent.change(await screen.findByTestId('totp-code'), { target: { value: '123456' } });
    fireEvent.click(screen.getByTestId('totp-confirm'));

    const codes = await screen.findByTestId('totp-recovery-codes');
    expect(codes).toHaveTextContent('aaaaa-bbbbb');
    expect(codes).toHaveTextContent('ccccc-ddddd');
  });

  it('refuses to let an admin remove a factor the instance requires', async () => {
    get.mockResolvedValue({ enrolled: true, recovery_codes_remaining: 8, required: true });
    renderPanel();

    await waitFor(() => expect(screen.getByTestId('totp-disable')).toBeDisabled());
    expect(screen.getByText(/8 recovery codes left/)).toBeInTheDocument();
  });

  it('asks for a current code before turning the factor off', async () => {
    get.mockResolvedValue({ enrolled: true, recovery_codes_remaining: 10, required: false });
    post.mockResolvedValue({ status: 'disabled' });
    renderPanel();

    fireEvent.click(await screen.findByTestId('totp-disable'));
    expect(screen.getByTestId('totp-disable-confirm')).toBeDisabled();

    fireEvent.change(screen.getByTestId('totp-disable-code'), { target: { value: '654321' } });
    fireEvent.click(screen.getByTestId('totp-disable-confirm'));

    await waitFor(() => expect(post).toHaveBeenCalledWith('/auth/totp/disable', { code: '654321' }));
  });
});
