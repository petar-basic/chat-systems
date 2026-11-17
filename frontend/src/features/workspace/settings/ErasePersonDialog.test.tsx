import { afterEach, describe, it, expect, vi } from 'vitest';
import i18n from '@/lib/i18n';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { ErasePersonDialog } from './ErasePersonDialog';

function renderDialog() {
  const onErase = vi.fn().mockResolvedValue(undefined);
  const onClose = vi.fn();
  render(<ErasePersonDialog name="Bob Smith" email="bob@dev.local" onErase={onErase} onClose={onClose} />);
  return { onErase, onClose };
}

describe('ErasePersonDialog', () => {
  it('names what goes', () => {
    renderDialog();
    const list = screen.getByTestId('erase-person-consequences');
    expect(list).toHaveTextContent('bob@dev.local');
    expect(list).toHaveTextContent('signed out everywhere');
    expect(list).toHaveTextContent('passkeys, authenticator, recovery codes, notification keywords');
    expect(list).toHaveTextContent('messages stay readable');
  });

  it('erases only once their name is typed', async () => {
    const { onErase, onClose } = renderDialog();
    const confirm = screen.getByTestId('erase-person-dialog-confirm');
    expect(confirm).toBeDisabled();

    fireEvent.change(screen.getByLabelText('Type Bob Smith to confirm'), { target: { value: 'Bob' } });
    expect(confirm).toBeDisabled();

    fireEvent.change(screen.getByLabelText('Type Bob Smith to confirm'), { target: { value: 'Bob Smith' } });
    expect(confirm).toBeEnabled();
    fireEvent.click(confirm);

    await waitFor(() => expect(onErase).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(onClose).toHaveBeenCalled());
  });
});

describe('ErasePersonDialog in Japanese', () => {
  const hostile = '<img src=x onerror="alert(1)"> {{name}}';

  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('names the person and what goes, all as text', async () => {
    await i18n.changeLanguage('ja');
    render(
      <ErasePersonDialog name={hostile} email={`${hostile}@dev.local`} onErase={vi.fn()} onClose={vi.fn()} />,
    );

    expect(screen.getByText(`${hostile} さんのデータを消去しますか?`)).toBeInTheDocument();
    expect(
      screen.getByText('このサーバーのすべてのワークスペースが対象です。元に戻すことはできません。'),
    ).toBeInTheDocument();
    const items = screen.getAllByRole('listitem').map((item) => item.textContent);
    expect(items).toEqual([
      `名前、メールアドレス(${hostile}@dev.local)、プロフィール画像、自己紹介は Deleted user に置き換わります。`,
      'すべての端末でサインアウトされ、以後サインインできなくなります。',
      '接続しているアプリは、すべて接続が解除されます。',
      '写真、パスキー、認証アプリ、リカバリーコード、通知キーワード、プッシュ通知の登録、サイドバーのセクション、各種設定は削除されます。',
      'メッセージは引き続き読めます。送信者は Deleted user と表示されます。',
    ]);
    expect(screen.getByText('Deleted user').tagName).toBe('EM');
    expect(screen.getByLabelText(`確認のため ${hostile} と入力してください`)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'データを消去' })).toBeDisabled();
    expect(document.querySelector('[onerror]')).toBeNull();
  });

  it('keeps tags in the address as text', async () => {
    await i18n.changeLanguage('ja');
    const email = '</email><em>x</em>{{email}}@dev.local';
    render(<ErasePersonDialog name="Bob" email={email} onErase={vi.fn()} onClose={vi.fn()} />);
    expect(screen.getByTestId('erase-person-email').textContent).toBe(email);
    expect(screen.getByTestId('erase-person-consequences').querySelectorAll('em')).toHaveLength(1);
  });

  it('says when there is no address', async () => {
    await i18n.changeLanguage('ja');
    render(<ErasePersonDialog name="Bob" email="" onErase={vi.fn()} onClose={vi.fn()} />);
    expect(screen.getByTestId('erase-person-email')).toHaveTextContent('なし');
  });
});
