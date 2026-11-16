import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import i18n from '@/lib/i18n';
import type { ProfileField } from '@/features/people';
import { ProfileFieldsSection } from './ProfileFieldsSection';

const hostile = '<img src=x onerror="alert(1)"> {{name}}';

const state = vi.hoisted(() => ({ fields: [] as ProfileField[] }));

vi.mock('@/features/people', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@/features/people')>()),
  useProfileFields: () => ({ fields: state.fields, isLoading: false }),
  useOrderProfileFields: () => ({ mutate: vi.fn() }),
  useDeleteProfileField: () => ({ mutateAsync: vi.fn() }),
}));

const field = (over: Partial<ProfileField> = {}): ProfileField => ({
  id: 'f1',
  workspace_id: 'w1',
  label: 'Department',
  kind: 'text',
  admins_only: false,
  on_card: false,
  options: [],
  position: 0,
  value_count: 0,
  ...over,
});

function leaves(value: unknown, prefix = ''): string[] {
  if (typeof value !== 'object' || value === null) return [prefix];
  return Object.entries(value).flatMap(([key, child]) => leaves(child, prefix ? `${prefix}.${key}` : key));
}

const counted = (keys: string[]) =>
  [...new Set(keys.map((key) => key.replace(/_(zero|one|other)$/, '')))].sort();

describe('profile fields in Japanese', () => {
  beforeEach(async () => {
    await i18n.changeLanguage('ja');
  });

  afterEach(async () => {
    state.fields = [];
    await i18n.changeLanguage('en');
  });

  it('says when there are no fields', () => {
    render(<ProfileFieldsSection workspaceId="w1" />);
    expect(screen.getByText('プロフィール項目はまだありません。')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '項目を追加' })).toBeInTheDocument();
  });

  it('lists the fields with their kind and badges, and their labels as text', () => {
    state.fields = [
      field({ label: hostile, admins_only: true, on_card: true }),
      field({ id: 'f2', label: 'Manager', kind: 'person', position: 1 }),
    ];
    render(<ProfileFieldsSection workspaceId="w1" />);

    expect(screen.getByText(hostile)).toBeInTheDocument();
    expect(screen.getByText('テキスト')).toBeInTheDocument();
    expect(screen.getByText('メンバー')).toBeInTheDocument();
    expect(screen.getByText('管理者のみ')).toBeInTheDocument();
    expect(screen.getByText('カードに表示')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: `${hostile} を編集` })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: `${hostile} を移動` })).toBeInTheDocument();
    expect(document.querySelector('[onerror]')).toBeNull();
  });

  it('says how many values go with a field before deleting it', () => {
    state.fields = [field({ label: hostile, value_count: 3 }), field({ id: 'f2', label: 'Office' })];
    render(<ProfileFieldsSection workspaceId="w1" />);

    fireEvent.click(screen.getByRole('button', { name: `${hostile} を削除` }));
    expect(screen.getByText(`${hostile} を削除しますか?`)).toBeInTheDocument();
    expect(
      screen.getByText('すべてのプロフィールから、入力済みの3件の値とともに削除されます。'),
    ).toBeInTheDocument();
    expect(document.querySelector('[onerror]')).toBeNull();
  });

  it('says when nobody has filled a field in yet', () => {
    state.fields = [field({ label: 'Office' })];
    render(<ProfileFieldsSection workspaceId="w1" />);

    fireEvent.click(screen.getByRole('button', { name: 'Office を削除' }));
    expect(
      screen.getByText('すべてのプロフィールから削除されます。まだ誰も入力していません。'),
    ).toBeInTheDocument();
  });

  it('has Japanese for every guest, invite request and profile field string', () => {
    for (const group of ['guest', 'inviteRequest', 'profileFields']) {
      const english = leaves(i18n.getResource('en', 'workspace', group));
      const japanese = leaves(i18n.getResource('ja', 'workspace', group));
      expect(counted(japanese)).toEqual(counted(english));
    }
  });
});
