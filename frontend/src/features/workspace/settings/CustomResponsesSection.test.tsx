import { afterEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, within } from '@testing-library/react';
import i18n from '@/lib/i18n';
import type { CustomResponse, CustomResponses } from '@/features/custom-responses';
import { CustomResponsesSection } from './CustomResponsesSection';

const hostile = '<img src=x onerror="alert(1)"> {{name}}';

const state = vi.hoisted(() => ({
  page: undefined as CustomResponses | undefined,
  save: vi.fn(),
  remove: vi.fn(),
  rename: vi.fn(),
}));

vi.mock('@/features/custom-responses', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@/features/custom-responses')>()),
  useCustomResponses: () => ({ data: state.page, isLoading: false, isError: false, refetch: vi.fn() }),
  useSaveCustomResponse: () => ({ mutate: state.save, isPending: false }),
  useDeleteCustomResponse: () => ({ mutateAsync: state.remove }),
  useRenameResponsesBot: () => ({ mutate: state.rename, isPending: false }),
}));

const response = (over: Partial<CustomResponse> = {}): CustomResponse => ({
  id: 'r1',
  workspace_id: 'w1',
  triggers: ['wifi password', 'wifi'],
  answers: ["It's on the fridge", 'Ask IT'],
  created_at: '2026-11-16T10:00:00Z',
  updated_at: '2026-11-16T10:00:00Z',
  ...over,
});

const page = (over: Partial<CustomResponses> = {}): CustomResponses => ({
  data: [response()],
  can_edit: true,
  can_rename: false,
  bot: { user_id: 'bot-1', name: 'Northwind', avatar_url: null },
  ...over,
});

describe('CustomResponsesSection', () => {
  afterEach(() => {
    state.page = undefined;
    state.save.mockReset();
    state.remove.mockReset();
    state.rename.mockReset();
  });

  it('lists each response with its phrases as text and the bot that answers', () => {
    state.page = page({ data: [response({ triggers: [hostile] })] });
    render(<CustomResponsesSection workspaceId="w1" />);

    const bot = screen.getByTestId('custom-responses-bot');
    expect(within(bot).getByText('Northwind')).toBeInTheDocument();
    expect(within(bot).getByText('App')).toBeInTheDocument();
    expect(screen.getByText(hostile)).toBeInTheDocument();
    expect(screen.getByText("It's on the fridge · 1 more answer")).toBeInTheDocument();
    expect(screen.getByRole('button', { name: `Edit the response to ${hostile}` })).toBeInTheDocument();
    expect(screen.queryByTestId('custom-responses-rename')).not.toBeInTheDocument();
    expect(document.querySelector('[onerror]')).toBeNull();
  });

  it('shows the responses without the controls when the rule keeps them with admins', () => {
    state.page = page({ can_edit: false });
    render(<CustomResponsesSection workspaceId="w1" />);

    expect(screen.getByTestId('custom-responses-read-only')).toHaveTextContent(
      'Only admins can edit custom responses in this workspace.',
    );
    expect(screen.queryByTestId('custom-responses-add')).not.toBeInTheDocument();
    expect(screen.queryByTestId('custom-responses-edit')).not.toBeInTheDocument();
    expect(screen.queryByTestId('custom-responses-delete')).not.toBeInTheDocument();
  });

  it('says when there are none yet', () => {
    state.page = page({ data: [] });
    render(<CustomResponsesSection workspaceId="w1" />);
    expect(screen.getByTestId('custom-responses-empty')).toHaveTextContent('No custom responses yet.');
  });

  it('saves the filled phrases and answers of a new response', () => {
    state.page = page({ data: [] });
    render(<CustomResponsesSection workspaceId="w1" />);

    fireEvent.click(screen.getByTestId('custom-responses-add'));
    const save = screen.getByTestId('custom-response-save');
    expect(save).toBeDisabled();

    fireEvent.change(screen.getByLabelText('Phrase 1'), { target: { value: '  wifi password ' } });
    fireEvent.click(screen.getByTestId('custom-response-add-trigger'));
    fireEvent.change(screen.getByLabelText('Phrase 2'), { target: { value: '   ' } });
    expect(save).toBeDisabled();
    fireEvent.change(screen.getByLabelText('Answer 1'), { target: { value: "It's on the fridge" } });
    expect(save).toBeEnabled();
    fireEvent.click(save);

    expect(state.save).toHaveBeenCalledWith(
      { id: null, body: { triggers: ['wifi password'], answers: ["It's on the fridge"] } },
      expect.anything(),
    );
  });

  it('edits a response in place', () => {
    state.page = page();
    render(<CustomResponsesSection workspaceId="w1" />);

    fireEvent.click(screen.getByRole('button', { name: 'Edit the response to wifi password' }));
    expect(screen.getByLabelText('Phrase 2')).toHaveValue('wifi');
    fireEvent.click(screen.getByRole('button', { name: 'Remove answer 2' }));
    fireEvent.click(screen.getByTestId('custom-response-save'));

    expect(state.save).toHaveBeenCalledWith(
      { id: 'r1', body: { triggers: ['wifi password', 'wifi'], answers: ["It's on the fridge"] } },
      expect.anything(),
    );
  });

  it('deletes a response once confirmed', async () => {
    state.page = page();
    state.remove.mockResolvedValue(undefined);
    render(<CustomResponsesSection workspaceId="w1" />);

    fireEvent.click(screen.getByRole('button', { name: 'Delete the response to wifi password' }));
    const dialog = screen.getByTestId('custom-responses-delete-dialog');
    expect(within(dialog).getByText('Northwind stops answering its phrases.')).toBeInTheDocument();
    fireEvent.click(within(dialog).getByRole('button', { name: 'Delete response' }));
    expect(state.remove).toHaveBeenCalledWith('r1');
  });

  it('lets an admin rename the bot', () => {
    state.page = page({ can_rename: true });
    render(<CustomResponsesSection workspaceId="w1" />);

    fireEvent.click(screen.getByTestId('custom-responses-rename'));
    const save = screen.getByTestId('custom-responses-rename-save');
    expect(save).toBeDisabled();
    fireEvent.change(screen.getByLabelText('Name'), { target: { value: '  Helpdesk ' } });
    fireEvent.click(save);
    expect(state.rename).toHaveBeenCalledWith('Helpdesk', expect.anything());
  });
});

describe('CustomResponsesSection in Japanese', () => {
  afterEach(async () => {
    state.page = undefined;
    state.remove.mockReset();
    await i18n.changeLanguage('en');
  });

  it('explains the responses, and names the bot and the phrases as text', async () => {
    state.page = page({
      data: [response({ triggers: [hostile] })],
      can_rename: true,
      bot: { user_id: 'bot-1', name: hostile, avatar_url: null },
    });
    await i18n.changeLanguage('ja');
    render(<CustomResponsesSection workspaceId="w1" />);

    expect(
      screen.getByText(
        `チャンネルやスレッドで誰かがレスポンスのフレーズを書くと、${hostile} がそのレスポンスの返答の中から1つを返します。1つのレスポンスが同じチャンネルで返答するのは1分に1回までで、ダイレクトメッセージでは返答しません。`,
      ),
    ).toBeInTheDocument();
    const bot = screen.getByTestId('custom-responses-bot');
    expect(within(bot).getByText('返答するボット')).toBeInTheDocument();
    expect(within(bot).getByText('アプリ')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '名前を変更' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'レスポンスを追加' })).toBeInTheDocument();
    expect(screen.getByText("It's on the fridge · ほかに返答 1件")).toBeInTheDocument();
    expect(screen.getByRole('button', { name: `「${hostile}」へのレスポンスを編集` })).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: `「${hostile}」へのレスポンスを削除` }));
    const dialog = screen.getByTestId('custom-responses-delete-dialog');
    expect(within(dialog).getByText(`「${hostile}」へのレスポンスを削除しますか?`)).toBeInTheDocument();
    expect(
      within(dialog).getByText(`${hostile} は、このレスポンスのフレーズに返答しなくなります。`),
    ).toBeInTheDocument();
    expect(within(dialog).getByRole('button', { name: 'レスポンスを削除' })).toBeInTheDocument();
    expect(document.querySelector('[onerror]')).toBeNull();
  });

  it('names every part of the response form', async () => {
    state.page = page({ bot: { user_id: 'bot-1', name: hostile, avatar_url: null } });
    await i18n.changeLanguage('ja');
    render(<CustomResponsesSection workspaceId="w1" />);

    fireEvent.click(screen.getByRole('button', { name: '「wifi password」へのレスポンスを編集' }));
    expect(screen.getByText('カスタムレスポンスを編集')).toBeInTheDocument();
    expect(screen.getByText('このフレーズが書かれたとき')).toBeInTheDocument();
    expect(
      screen.getByText(
        'フレーズは単語単位で一致し、大文字と小文字は区別しません。日本語のように単語を空白で区切らない文では、文中のどこにあっても一致します。',
      ),
    ).toBeInTheDocument();
    expect(screen.getByLabelText('フレーズ 2')).toHaveValue('wifi');
    expect(screen.getByLabelText('フレーズ 1')).toHaveAttribute('placeholder', '例: Wi-Fi のパスワード');
    expect(screen.getByRole('button', { name: 'フレーズ 2 を削除' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'フレーズを追加' })).toBeInTheDocument();
    expect(screen.getByText(`${hostile} の返答`)).toBeInTheDocument();
    expect(
      screen.getByText('返答が複数あるときは、毎回その中から1つがランダムに選ばれます。'),
    ).toBeInTheDocument();
    expect(screen.getByLabelText('返答 1')).toHaveAttribute('placeholder', '例: 冷蔵庫に貼ってあります');
    expect(screen.getByRole('button', { name: '返答 2 を削除' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '返答を追加' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'レスポンスを保存' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'キャンセル' })).toBeInTheDocument();
    expect(document.querySelector('[onerror]')).toBeNull();
  });

  it('says when there are none, and when only admins edit', async () => {
    state.page = page({ data: [], can_edit: false });
    await i18n.changeLanguage('ja');
    render(<CustomResponsesSection workspaceId="w1" />);

    expect(screen.getByTestId('custom-responses-empty')).toHaveTextContent(
      'カスタムレスポンスはまだありません。',
    );
    expect(screen.getByTestId('custom-responses-read-only')).toHaveTextContent(
      'このワークスペースでは、カスタムレスポンスを編集できるのは管理者だけです。',
    );
  });

  it('names the rename dialog', async () => {
    state.page = page({ can_rename: true });
    await i18n.changeLanguage('ja');
    render(<CustomResponsesSection workspaceId="w1" />);

    fireEvent.click(screen.getByTestId('custom-responses-rename'));
    expect(screen.getByText('ボットの名前を変更')).toBeInTheDocument();
    expect(screen.getByLabelText('名前')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '名前を保存' })).toBeInTheDocument();
  });
});

describe('custom responses strings', () => {
  it('has Japanese for every one', () => {
    const base = (key: string) => key.replace(/_(one|other)$/, '');
    const keys = (lang: string) =>
      [...new Set(Object.keys(i18n.getResource(lang, 'workspace', 'customResponses')).map(base))].sort();
    expect(keys('ja')).toEqual(keys('en'));
  });
});
