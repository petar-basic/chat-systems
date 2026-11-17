import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import i18n from '@/lib/i18n';
import type { UserGroup } from '@/features/workspace/hooks/useUserGroups';
import { UserGroupsSection } from './UserGroupsSection';

const hostile = '<img src=x onerror="alert(1)"> {{name}}';

const state = vi.hoisted(() => ({ groups: [] as UserGroup[] }));

vi.mock('@/features/workspace/hooks/useUserGroups', () => ({
  useUserGroups: () => ({ data: state.groups, isLoading: false }),
  useCreateGroup: () => ({ mutateAsync: vi.fn(), isPending: false }),
  useDeleteGroup: () => ({ mutate: vi.fn() }),
  useSetGroupMember: () => ({ mutateAsync: vi.fn() }),
}));

const group = (over: Partial<UserGroup>) =>
  ({ id: 'g1', handle: 'design', member_count: 1, ...over }) as UserGroup;

function renderSection() {
  render(<UserGroupsSection workspaceId="w1" members={[]} isAdmin />);
}

describe('UserGroupsSection', () => {
  beforeEach(() => {
    state.groups = [];
  });

  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('counts people in English as before', () => {
    state.groups = [group({}), group({ id: 'g2', handle: 'ops', member_count: 3 })];
    renderSection();

    expect(screen.getByText('1 person')).toBeInTheDocument();
    expect(screen.getByText('3 people')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Delete @design' })).toBeInTheDocument();
  });

  it('says it all in Japanese, and names a group as text', async () => {
    state.groups = [group({ handle: hostile }), group({ id: 'g2', handle: 'ops', member_count: 3 })];
    await i18n.changeLanguage('ja');
    renderSection();

    expect(screen.getByLabelText('グループのハンドル')).toHaveAttribute(
      'placeholder',
      'ハンドル(@handle の形でメンションします)',
    );
    expect(screen.getByText(`@${hostile}`)).toBeInTheDocument();
    expect(screen.getByText('1人')).toBeInTheDocument();
    expect(screen.getByText('3人')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: `@${hostile} を削除` })).toBeInTheDocument();
    expect(document.querySelector('[onerror]')).toBeNull();
  });

  it('says when there are none in Japanese', async () => {
    await i18n.changeLanguage('ja');
    renderSection();

    expect(
      screen.getByText(
        'グループはまだありません。グループを作ると、チームを1つのハンドルでメンションできます。',
      ),
    ).toBeInTheDocument();
  });
});
