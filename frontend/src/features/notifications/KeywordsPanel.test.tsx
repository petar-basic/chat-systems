import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { KeywordsPanel } from './KeywordsPanel';

const save = vi.fn();
let keywords: string[] = [];

vi.mock('@/stores/workspace', () => ({
  useWorkspaceStore: (select: (state: unknown) => unknown) =>
    select({ currentWorkspace: { id: 'ws-1', instanceUrl: 'http://localhost:3000' } }),
}));
vi.mock('@/hooks/queries/useNotificationKeywords', () => ({
  useNotificationKeywords: () => ({ data: keywords }),
  useSetNotificationKeywords: () => ({ mutate: save }),
}));

const input = () => screen.getByRole('textbox', { name: 'My keywords' });

describe('KeywordsPanel', () => {
  beforeEach(() => {
    save.mockReset();
    keywords = ['deploy'];
  });

  it('adds a keyword on Enter and on a comma, and clears the field', () => {
    render(<KeywordsPanel />);
    fireEvent.change(input(), { target: { value: ' outage ' } });
    fireEvent.keyDown(input(), { key: 'Enter' });
    expect(save).toHaveBeenCalledWith(['deploy', 'outage'], expect.anything());
    expect(input()).toHaveValue('');

    fireEvent.change(input(), { target: { value: 'on call' } });
    fireEvent.keyDown(input(), { key: ',' });
    expect(save).toHaveBeenLastCalledWith(['deploy', 'on call'], expect.anything());
  });

  it('says why a keyword is refused and keeps what was typed', () => {
    render(<KeywordsPanel />);
    fireEvent.change(input(), { target: { value: 'Deplóy' } });
    fireEvent.keyDown(input(), { key: 'Enter' });
    expect(save).not.toHaveBeenCalled();
    expect(screen.getByRole('alert')).toHaveTextContent('already on your list');
    expect(input()).toHaveValue('Deplóy');
    expect(input()).toHaveAttribute('aria-invalid', 'true');
  });

  it('removes a keyword from its chip', () => {
    keywords = ['deploy', 'outage'];
    render(<KeywordsPanel />);
    expect(screen.getAllByTestId('notification-keyword').map((chip) => chip.textContent)).toEqual([
      'deploy',
      'outage',
    ]);
    fireEvent.click(screen.getByRole('button', { name: 'Remove deploy' }));
    expect(save).toHaveBeenCalledWith(['outage'], expect.anything());
  });
});
