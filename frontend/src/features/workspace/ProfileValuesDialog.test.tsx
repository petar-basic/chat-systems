import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import i18n from '@/lib/i18n';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import type { ProfileField } from '@/features/people';
import { ProfileValuesDialog } from './ProfileValuesDialog';

const save = vi.hoisted(() => ({ mutate: vi.fn(), isPending: false }));

vi.mock('@/features/people', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@/features/people')>()),
  useSetProfileValues: () => save,
}));

const hostile = '<img src=x onerror="alert(1)"> {{name}}';

const field = (id: string, kind: ProfileField['kind']): ProfileField => ({
  id,
  workspace_id: 'ws',
  label: id,
  kind,
  options: kind === 'options' ? [{ id: 'fin', label: 'Finance' }] : [],
  admins_only: false,
  on_card: false,
  position: 0,
  value_count: 0,
});

function renderDialog() {
  render(
    <ProfileValuesDialog
      userId="u-ana"
      name={hostile}
      workspaceId="ws"
      fields={[field('dept', 'options'), field('manager', 'person')]}
      values={{}}
      onClose={vi.fn()}
    />,
  );
}

describe('ProfileValuesDialog in Japanese', () => {
  beforeEach(async () => {
    save.mutate.mockReset();
    await i18n.changeLanguage('ja');
  });

  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('names whose fields they are, with the name as text', () => {
    renderDialog();
    expect(screen.getByRole('heading', { name: `${hostile} さんのプロフィール項目` })).toBeInTheDocument();
    expect(document.querySelector('[onerror]')).toBeNull();
    expect(screen.getByTestId('profile-field-dept')).toHaveDisplayValue('未設定');
    expect(screen.getByTestId('profile-field-manager')).toHaveValue('指定なし');
    expect(screen.getByTestId('profile-values-save')).toHaveTextContent('保存');
  });

  it('says the fields could not be saved when the server gives no reason', async () => {
    save.mutate.mockImplementation((_vars, options: { onError: (err: unknown) => void }) =>
      options.onError({}),
    );
    renderDialog();
    fireEvent.change(screen.getByTestId('profile-field-dept'), { target: { value: 'fin' } });
    fireEvent.click(screen.getByTestId('profile-values-save'));
    await waitFor(() =>
      expect(screen.getByRole('alert')).toHaveTextContent('プロフィール項目を保存できませんでした。'),
    );
  });
});
