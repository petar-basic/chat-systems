import { afterEach, describe, expect, it, vi } from 'vitest';
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { BrowserRouter, useNavigate } from 'react-router';
import i18n from '@/lib/i18n';
import { DesktopTopBar } from './DesktopTopBar';

function Go() {
  const navigate = useNavigate();
  return (
    <button type="button" data-qa="go" onClick={() => void navigate('/app/w1/c2')}>
      go
    </button>
  );
}

function renderBar(onSearch = vi.fn()) {
  render(
    <BrowserRouter>
      <DesktopTopBar workspaceName="Acme" searching={false} onSearch={onSearch} />
      <Go />
    </BrowserRouter>,
  );
  return onSearch;
}

describe('DesktopTopBar', () => {
  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('names the workspace it searches and opens search', () => {
    const onSearch = renderBar();
    fireEvent.click(screen.getByTestId('desktop-search'));
    expect(screen.getByTestId('desktop-search')).toHaveTextContent('Search Acme');
    expect(onSearch).toHaveBeenCalledOnce();
  });

  it('is a drag region for the window', () => {
    renderBar();
    expect(screen.getByTestId('desktop-top-bar')).toHaveAttribute('data-tauri-drag-region', 'deep');
  });

  it('goes back only after a step and forward only after going back', async () => {
    window.history.replaceState({ idx: 0 }, '', '/app/w1/c1');
    renderBar();
    expect(screen.getByTestId('desktop-back')).toBeDisabled();
    expect(screen.getByTestId('desktop-forward')).toBeDisabled();

    fireEvent.click(screen.getByTestId('go'));
    await waitFor(() => expect(screen.getByTestId('desktop-back')).toBeEnabled());
    expect(screen.getByTestId('desktop-forward')).toBeDisabled();

    act(() => fireEvent.click(screen.getByTestId('desktop-back')));
    await waitFor(() => expect(screen.getByTestId('desktop-forward')).toBeEnabled());
    expect(screen.getByTestId('desktop-back')).toBeDisabled();
  });

  it('speaks Japanese', async () => {
    await i18n.changeLanguage('ja');
    renderBar();
    expect(screen.getByTestId('desktop-search')).toHaveTextContent('Acmeを検索');
    expect(screen.getByRole('button', { name: '戻る' })).toHaveAttribute('data-tooltip', '戻る');
    expect(screen.getByRole('button', { name: '進む' })).toHaveAttribute('data-tooltip', '進む');
  });
});
