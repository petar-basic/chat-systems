import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import i18n from '@/lib/i18n';
import { QueryState } from './QueryState';

describe('QueryState', () => {
  it('shows a spinner while loading and hides children', () => {
    render(
      <QueryState isLoading isError={false}>
        <div>child</div>
      </QueryState>,
    );
    expect(screen.getByTestId('query-loading')).toBeInTheDocument();
    expect(screen.queryByText('child')).not.toBeInTheDocument();
  });

  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('offers to retry in the language of the page', async () => {
    await i18n.changeLanguage('ja');
    render(
      <QueryState isLoading={false} isError onRetry={vi.fn()}>
        <p>content</p>
      </QueryState>,
    );
    expect(screen.getByRole('button', { name: '再試行' })).toBeInTheDocument();
  });

  it('shows an error with a working retry button', () => {
    const onRetry = vi.fn();
    render(
      <QueryState isLoading={false} isError onRetry={onRetry}>
        <div>child</div>
      </QueryState>,
    );
    expect(screen.getByTestId('query-error')).toBeInTheDocument();
    fireEvent.click(screen.getByTestId('query-retry'));
    expect(onRetry).toHaveBeenCalledTimes(1);
  });

  it('shows the empty slot only when empty (not on a failed fetch)', () => {
    render(
      <QueryState isLoading={false} isError={false} isEmpty empty={<span>nothing here</span>}>
        <div>child</div>
      </QueryState>,
    );
    expect(screen.getByText('nothing here')).toBeInTheDocument();
    expect(screen.queryByText('child')).not.toBeInTheDocument();
  });

  it('renders children on success', () => {
    render(
      <QueryState isLoading={false} isError={false}>
        <div>child</div>
      </QueryState>,
    );
    expect(screen.getByText('child')).toBeInTheDocument();
  });

  it.each([
    ['en', 'Something went wrong. Please try again.'],
    ['ja', '問題が発生しました。もう一度お試しください。'],
  ])('says something went wrong when it has no message (%s)', async (language, generic) => {
    await i18n.changeLanguage(language);
    render(
      <QueryState isLoading={false} isError>
        <div>child</div>
      </QueryState>,
    );
    expect(screen.getByTestId('query-error')).toHaveTextContent(generic);
  });

  it('shows the message it is given as text', () => {
    const hostile = '<img src=x onerror="alert(1)"> {{name}}';
    render(
      <QueryState isLoading={false} isError errorMessage={hostile}>
        <div>child</div>
      </QueryState>,
    );
    expect(screen.getByText(hostile)).toBeInTheDocument();
    expect(document.querySelector('[onerror]')).toBeNull();
  });
});
