import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import i18n from '@/lib/i18n';
import { KeywordsPanel } from './KeywordsPanel';

const save = vi.fn();
let keywords: string[] = [];

vi.mock('@/stores/workspace', () => ({
  useWorkspaceStore: (select: (state: unknown) => unknown) =>
    select({ currentWorkspace: { id: 'ws-1', instanceUrl: 'http://localhost:3000' } }),
}));
vi.mock('@/features/notifications/hooks/useNotificationKeywords', () => ({
  useNotificationKeywords: () => ({ data: keywords }),
  useSetNotificationKeywords: () => ({ mutate: save }),
}));

const input = () => screen.getByRole('textbox', { name: 'My keywords' });

describe('KeywordsPanel', () => {
  beforeEach(() => {
    save.mockReset();
    keywords = ['deploy'];
  });

  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('in Japanese says why a keyword is refused, with the keyword as text', async () => {
    const hostile = '<img src=x onerror="alert(1)">';
    keywords = [hostile];
    await i18n.changeLanguage('ja');
    render(<KeywordsPanel />);
    const field = screen.getByRole('textbox', { name: '通知するキーワード' });

    fireEvent.change(field, { target: { value: hostile } });
    fireEvent.keyDown(field, { key: 'Enter' });

    expect(screen.getByRole('alert')).toHaveTextContent(`「${hostile}」はすでに登録されています。`);
    expect(screen.getByRole('button', { name: `${hostile} を削除` })).toBeInTheDocument();
    expect(document.querySelector('img')).toBeNull();
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
