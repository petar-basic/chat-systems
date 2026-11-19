import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, waitFor, within } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import i18n from '@/lib/i18n';
import type { ProfileField } from '@/features/people';
import { EditProfileModal } from './EditProfileModal';

const saveProfile = vi.fn();
const setStatus = vi.fn();
const clearStatus = vi.fn();
const setValues = vi.fn();
const upload = vi.fn();
const hostile = '<img src=x onerror="alert(1)"> {{name}}';
let workspaceName = 'Dev Team';
let account: Record<string, string | null> = {};
let profile: Record<string, string | null> = {};
let fields: ProfileField[] = [];

const field = (id: string, label: string, admins_only = false): ProfileField => ({
  id,
  workspace_id: 'ws-1',
  label,
  kind: 'text',
  options: [],
  admins_only,
  on_card: false,
  position: 0,
  value_count: 0,
});

vi.mock('@/features/instance/hooks/useAuth', () => ({
  useCurrentUser: () => ({ data: { id: 'user-1', email: 'alice@dev.local' } }),
}));
vi.mock('@/features/profile/hooks/useProfile', () => ({
  useAccount: () => ({ data: account }),
  useWorkspaceProfile: () => ({ data: profile, isLoading: false }),
  useSaveWorkspaceProfile: () => ({
    profile: { mutateAsync: (...args: unknown[]) => saveProfile(...args) },
    setStatus: { mutateAsync: (...args: unknown[]) => setStatus(...args) },
    clearStatus: { mutateAsync: (...args: unknown[]) => clearStatus(...args) },
    refresh: vi.fn(),
  }),
}));
vi.mock('@/features/people/hooks/useProfileFields', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@/features/people/hooks/useProfileFields')>()),
  useProfileFields: () => ({ fields }),
  useSetProfileValues: () => ({ mutateAsync: (...args: unknown[]) => setValues(...args) }),
}));
vi.mock('@/features/profile/hooks/usePhotoUpload', () => ({
  usePhotoUpload: () => ({ upload: (...args: unknown[]) => upload(...args), uploading: false }),
}));
vi.mock('@/features/messaging', () => ({
  EmojiPicker: ({ onSelect, onClose }: { onSelect: (emoji: string) => void; onClose: () => void }) => (
    <button
      type="button"
      onClick={() => {
        onSelect(':shipit:');
        onClose();
      }}
    >
      pick shipit
    </button>
  ),
}));
vi.mock('@/stores/workspace', () => ({
  useWorkspaceStore: (select: (state: unknown) => unknown) =>
    select({ currentWorkspace: { id: 'ws-1', name: workspaceName, instanceUrl: 'http://localhost:3000' } }),
}));

function renderModal() {
  const onClose = vi.fn();
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <QueryClientProvider client={client}>
      <EditProfileModal onClose={onClose} />
    </QueryClientProvider>,
  );
  return onClose;
}

describe('EditProfileModal', () => {
  beforeEach(() => {
    saveProfile.mockReset().mockResolvedValue(undefined);
    setStatus.mockReset().mockResolvedValue(undefined);
    clearStatus.mockReset().mockResolvedValue(undefined);
    setValues.mockReset().mockResolvedValue(undefined);
    upload.mockReset().mockResolvedValue(null);
    workspaceName = 'Dev Team';
    fields = [];
    account = {
      display_name: 'Alice',
      title: 'Engineer',
      pronouns: null,
      bio: 'Backend at Acme',
      avatar_url: null,
    };
    profile = {
      workspace_id: 'ws-1',
      display_name: null,
      bio: null,
      avatar_url: null,
      status_emoji: null,
      status_text: null,
      status_expires_at: null,
    };
  });

  it('saves the profile and a new status with one button, then closes', async () => {
    const onClose = renderModal();

    fireEvent.change(screen.getByRole('textbox', { name: /Display name/ }), { target: { value: 'Alice J' } });
    fireEvent.change(screen.getByRole('textbox', { name: /Title/ }), {
      target: { value: ' Head of Finance ' },
    });
    fireEvent.click(screen.getByRole('button', { name: '🍕 At lunch' }));
    expect(screen.getByRole('combobox', { name: 'Clear after' })).toHaveValue('60');
    fireEvent.click(screen.getByRole('button', { name: 'Save changes' }));

    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(saveProfile).toHaveBeenCalledWith({
      display_name: 'Alice J',
      title: 'Head of Finance',
      pronouns: '',
      bio: '',
      avatar_url: '',
    });
    expect(setStatus).toHaveBeenCalledWith(expect.objectContaining({ emoji: '🍕', text: 'At lunch' }));
    expect(clearStatus).not.toHaveBeenCalled();
  });

  it("edits this workspace's profile and shows the account's where it has none", () => {
    renderModal();

    expect(screen.getByText('Dev Team')).toBeInTheDocument();
    const name = screen.getByRole('textbox', { name: /Display name/ });
    expect(name).toHaveValue('');
    expect(name).toHaveAttribute('placeholder', 'Alice');
    expect(screen.getByRole('textbox', { name: /Bio/ })).toHaveAttribute('placeholder', 'Backend at Acme');
    expect(screen.getByRole('textbox', { name: /Title/ })).toHaveAttribute('placeholder', 'Engineer');
  });

  it("goes back to the account's photo", async () => {
    account = { ...account, avatar_url: '/api/files/account.png' };
    profile = { ...profile, display_name: 'Ali', avatar_url: '/api/files/work.png' };
    const onClose = renderModal();

    fireEvent.click(screen.getByRole('button', { name: 'Use account photo' }));
    fireEvent.click(screen.getByRole('button', { name: 'Save changes' }));

    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(saveProfile).toHaveBeenCalledWith({
      display_name: 'Ali',
      title: '',
      pronouns: '',
      bio: '',
      avatar_url: '',
    });
  });

  it('picks the status emoji from the picker, a workspace one included', async () => {
    renderModal();

    fireEvent.click(screen.getByRole('button', { name: 'Choose a status emoji' }));
    fireEvent.click(screen.getByRole('button', { name: 'pick shipit' }));
    expect(screen.getByRole('button', { name: 'Status emoji :shipit:' })).toBeInTheDocument();
    fireEvent.change(screen.getByRole('textbox', { name: 'Status' }), { target: { value: 'Shipping' } });
    fireEvent.click(screen.getByRole('button', { name: 'Save changes' }));

    await waitFor(() =>
      expect(setStatus).toHaveBeenCalledWith(
        expect.objectContaining({ emoji: ':shipit:', text: 'Shipping' }),
      ),
    );
  });

  it('leaves an untouched status and its expiry alone', async () => {
    profile = {
      ...profile,
      status_emoji: '🍕',
      status_text: 'At lunch',
      status_expires_at: '2026-10-23T13:00:00Z',
    };
    const onClose = renderModal();

    expect(screen.getByTestId('status-expiry')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Save changes' }));

    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(setStatus).not.toHaveBeenCalled();
    expect(clearStatus).not.toHaveBeenCalled();
  });

  it('clears a status that was removed', async () => {
    profile = { ...profile, status_emoji: '🍕', status_text: 'At lunch' };
    const onClose = renderModal();

    fireEvent.click(screen.getByRole('button', { name: 'Clear status' }));
    fireEvent.click(screen.getByRole('button', { name: 'Save changes' }));

    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(clearStatus).toHaveBeenCalled();
    expect(setStatus).not.toHaveBeenCalled();
  });

  it('saves the profile fields a member may fill in, and only the changed ones', async () => {
    fields = [field('office', 'Office'), field('number', 'Employee number', true), field('pet', 'Pet')];
    const onClose = renderModal();

    expect(screen.queryByRole('textbox', { name: /Employee number/ })).not.toBeInTheDocument();
    fireEvent.change(screen.getByRole('textbox', { name: /Office/ }), { target: { value: ' Belgrade ' } });
    fireEvent.click(screen.getByRole('button', { name: 'Save changes' }));

    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(setValues).toHaveBeenCalledWith({ userId: 'user-1', values: { office: 'Belgrade' } });
  });

  it('stays open and shows a refused profile field by name', async () => {
    fields = [field('site', 'Website')];
    setValues.mockRejectedValue(new Error('Website: a link starting with http:// or https://'));
    const onClose = renderModal();

    fireEvent.change(screen.getByRole('textbox', { name: /Website/ }), {
      target: { value: 'bob.example.com' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Save changes' }));

    expect(await screen.findByRole('alert')).toHaveTextContent('Website: a link');
    expect(onClose).not.toHaveBeenCalled();
  });

  it('stays open and says why when saving fails', async () => {
    saveProfile.mockRejectedValue(new Error('Display name is too long'));
    const onClose = renderModal();

    fireEvent.click(screen.getByRole('button', { name: 'Save changes' }));

    expect(await screen.findByRole('alert')).toHaveTextContent('Display name is too long');
    expect(onClose).not.toHaveBeenCalled();
    expect(setStatus).not.toHaveBeenCalled();
  });

  it('offers how long a status lasts, in English', () => {
    renderModal();

    fireEvent.click(screen.getByRole('button', { name: '📅 In a meeting' }));
    const options = within(screen.getByRole('combobox', { name: 'Clear after' })).getAllByRole('option');
    expect(options.map((o) => o.textContent)).toEqual([
      "Don't clear",
      '30 minutes',
      '1 hour',
      '4 hours',
      'Today',
    ]);
  });

  it('says when the photo could not be uploaded', async () => {
    renderModal();

    fireEvent.change(screen.getByLabelText('Profile photo file'), {
      target: { files: [new File(['x'], 'me.png', { type: 'image/png' })] },
    });

    expect(await screen.findByRole('alert')).toHaveTextContent('Could not upload that photo.');
  });

  describe('in Japanese', () => {
    beforeEach(async () => {
      await i18n.changeLanguage('ja');
    });

    afterEach(async () => {
      await i18n.changeLanguage('en');
    });

    it('shows a hostile workspace, account and status as text', () => {
      workspaceName = hostile;
      account = {
        display_name: hostile,
        title: hostile,
        pronouns: hostile,
        bio: hostile,
        avatar_url: '/api/files/a.png',
      };
      profile = {
        ...profile,
        avatar_url: '/api/files/w.png',
        status_emoji: hostile,
        status_text: hostile,
        status_expires_at: '2026-10-23T13:00:00Z',
      };
      renderModal();

      expect(screen.getByText('プロフィールを編集')).toBeInTheDocument();
      const workspace = screen.getByText(hostile, { selector: 'strong' });
      expect(workspace.parentElement).toHaveTextContent(
        `${hostile} でのあなたのプロフィールです。空欄の項目には、アカウントの内容が使われます。`,
        { normalizeWhitespace: false },
      );
      expect(screen.getByRole('textbox', { name: /^表示名/ })).toHaveAttribute('placeholder', hostile);
      expect(screen.getByRole('textbox', { name: /^肩書き/ })).toHaveAttribute('placeholder', hostile);
      expect(screen.getByRole('textbox', { name: /^代名詞/ })).toHaveAttribute('placeholder', hostile);
      expect(screen.getByRole('textbox', { name: /^自己紹介/ })).toHaveAttribute('placeholder', hostile);
      expect(screen.getByRole('button', { name: `ステータスの絵文字 ${hostile}` })).toBeInTheDocument();
      expect(screen.getByRole('textbox', { name: 'ステータス' })).toHaveValue(hostile);
      expect(screen.getByRole('button', { name: 'ステータスをクリア' })).toBeInTheDocument();
      expect(screen.getByTestId('status-expiry')).toHaveTextContent(/^.+ にクリアされます$/);
      expect(screen.getByRole('button', { name: 'アカウントの写真を使う' })).toBeInTheDocument();
      expect(screen.getByRole('button', { name: 'キャンセル' })).toBeInTheDocument();
      expect(screen.getByRole('button', { name: '変更を保存' })).toBeInTheDocument();
      expect(document.querySelector('[onerror]')).toBeNull();
    });

    it('labels an empty profile and sets a status from a preset', async () => {
      account = { display_name: 'Alice', title: null, pronouns: null, bio: null, avatar_url: null };
      profile = { ...profile, avatar_url: '/api/files/w.png' };
      const onClose = renderModal();

      expect(screen.getByRole('textbox', { name: /^肩書き/ })).toHaveAttribute(
        'placeholder',
        '担当や役職。例: 経理部長',
      );
      expect(screen.getByRole('textbox', { name: /^代名詞/ })).toHaveAttribute(
        'placeholder',
        '例: they/them',
      );
      expect(screen.getByRole('textbox', { name: /^自己紹介/ })).toHaveAttribute(
        'placeholder',
        '担当している仕事、勤務地、連絡方法など',
      );
      expect(screen.getByRole('button', { name: '写真を削除' })).toBeInTheDocument();
      expect(screen.getByRole('textbox', { name: 'ステータス' })).toHaveAttribute(
        'placeholder',
        '今どうしていますか？',
      );
      expect(screen.getByRole('button', { name: 'ステータスの絵文字を選択' })).toBeInTheDocument();
      expect(screen.getAllByTestId('status-preset').map((b) => b.textContent)).toEqual([
        '📅 会議中',
        '🍕 昼休み中',
        '🤒 病欠',
        '🌴 休暇中',
      ]);

      fireEvent.click(screen.getByRole('button', { name: '🍕 昼休み中' }));
      const clearAfter = screen.getByRole('combobox', { name: 'クリアするまで' });
      expect(clearAfter).toHaveValue('60');
      expect(
        within(clearAfter)
          .getAllByRole('option')
          .map((o) => o.textContent),
      ).toEqual(['クリアしない', '30 分', '1 時間', '4 時間', '12 時間']);
      fireEvent.click(screen.getByRole('button', { name: '変更を保存' }));

      await waitFor(() => expect(onClose).toHaveBeenCalled());
      expect(setStatus).toHaveBeenCalledWith(expect.objectContaining({ emoji: '🍕', text: '昼休み中' }));
    });

    it('says the profile could not be saved when the server gives no reason', async () => {
      saveProfile.mockRejectedValue({});
      renderModal();

      fireEvent.click(screen.getByRole('button', { name: '変更を保存' }));

      expect(await screen.findByRole('alert')).toHaveTextContent('プロフィールを保存できませんでした。');
    });

    it('says the photo could not be uploaded', async () => {
      renderModal();

      fireEvent.change(screen.getByLabelText('プロフィール写真のファイル'), {
        target: { files: [new File(['x'], 'me.png', { type: 'image/png' })] },
      });

      expect(await screen.findByRole('alert')).toHaveTextContent('写真をアップロードできませんでした。');
    });
  });
});

describe('the edit profile strings', () => {
  function leaves(value: unknown, prefix = ''): string[] {
    if (typeof value !== 'object' || value === null) return [prefix];
    return Object.entries(value).flatMap(([key, child]) => leaves(child, prefix ? `${prefix}.${key}` : key));
  }

  it.each(['editProfile', 'status'])('has Japanese for every one in %s', (group) => {
    const english = leaves(i18n.getResource('en', 'profile', group)).filter((key) => !key.endsWith('_one'));
    const japanese = leaves(i18n.getResource('ja', 'profile', group));
    expect(english.length).toBeGreaterThan(0);
    expect(japanese).toEqual(english);
  });
});
