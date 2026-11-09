import { describe, expect, it, vi } from 'vitest';
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { BrowserRouter, useNavigate } from 'react-router';
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
});
