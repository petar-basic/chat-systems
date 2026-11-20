import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import i18n from '@/lib/i18n';
import { formatDay } from '@/lib/datetime';
import { notificationText, type NotificationData } from './notificationText';

const t = i18n.getFixedT(null, ['notifications', 'messages'] as const);
const text = (kind: string, data: NotificationData = {}, body: string | null = 'hello') =>
  notificationText(t, kind, data, body);

const sent: NotificationData = { sender_name: 'Ana', channel_name: 'general' };
const hostile = '<img src=x onerror="alert(1)"> {{name}}';

describe('notificationText', () => {
  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('writes a message notification from who sent it and where', () => {
    expect(text('mention', sent)).toEqual({ title: 'You were mentioned', body: 'hello' });
    expect(text('reply', sent).title).toBe('New reply in a thread');
    expect(text('dm', sent).title).toBe('Ana');
    expect(text('message', sent).title).toBe('Ana in #general');
    expect(text('dm').title).toBe('Somebody');
  });

  it('names the keyword, and the channel when there is one', () => {
    expect(text('keyword', { keyword: 'deploy', channel_name: 'ops' }).title).toBe(
      'Matched "deploy" in #ops',
    );
    expect(text('keyword', { keyword: 'deploy' }).title).toBe('Matched "deploy"');
  });

  it('tells an assignment from a mention on a list item', () => {
    const item: NotificationData = { sender_name: 'Ana', item_title: 'Fix login', list_title: 'Launch plan' };
    expect(text('list', { ...item, assigned: true }, null)).toEqual({
      title: 'Ana assigned you to Fix login',
      body: 'in Launch plan',
    });
    expect(text('list', item, 'take a look')).toEqual({
      title: 'Ana mentioned you on Fix login',
      body: 'take a look',
    });
    expect(text('list', { sender_name: 'Ana' }).title).toBe('Ana mentioned you on Untitled');
  });

  it('writes the words a call and a reminder used to get from the server', () => {
    expect(text('call', {}, null)).toEqual({
      title: 'Incoming huddle',
      body: 'Someone is starting a huddle',
    });
    expect(text('reminder', {}, null)).toEqual({ title: 'Reminder', body: 'A message you saved for later' });
    expect(text('reminder', {}, 'call the bank').body).toBe('call the bank');
  });

  it('says which system notice it is', () => {
    expect(
      text('system', { notice: 'workflow_stopped', workflow_name: 'Pager', stopped: 'left_workspace' }, null),
    ).toEqual({
      title: 'A workflow was turned off',
      body: '“Pager” stopped because the person who built it left the workspace.',
    });
    expect(
      text('system', { notice: 'workflow_app_gone', workflow_name: 'Triage', app_name: 'Ticketing' }, null),
    ).toEqual({
      title: "A workflow step can't run",
      body: '“Triage” uses a step from Ticketing, which is no longer available. Runs fail at that step until you replace it.',
    });
    expect(text('system', { notice: 'app_requested', sender_name: 'Bo' }, 'PagerDuty: alerts')).toEqual({
      title: 'Bo asked for an app',
      body: 'PagerDuty: alerts',
    });
    expect(text('system', { notice: 'not_delivered', failure: 'channel_archived' }, null)).toEqual({
      title: 'Scheduled message was not delivered',
      body: 'The channel was archived.',
    });
    expect(text('system', { notice: 'not_delivered', failure: 'internal_error' }, null).body).toBe(
      'The server could not deliver it.',
    );
    const until = '2026-11-16T12:00:00Z';
    expect(text('system', { notice: 'guest_ending', guest_name: 'Vera', guest_until: until }, 'raw')).toEqual(
      {
        title: "Vera's guest access is ending",
        body: `Their access ends on ${formatDay(until)}. Move or clear the date in Workspace settings → Members.`,
      },
    );
  });

  it('tells admins of an invite request and the requester of its answer', () => {
    const emails = ['a@x.test', 'b@x.test'];
    expect(
      text('system', { notice: 'invite_requested', sender_name: 'Rita' }, 'a@x.test, b@x.test: new hires'),
    ).toEqual({ title: 'Rita asked to invite people', body: 'a@x.test, b@x.test: new hires' });
    expect(
      text('system', { notice: 'invite_request_approved', sender_name: 'Ada', invite_emails: emails }),
    ).toEqual({
      title: 'Ada approved your invite request',
      body: 'For a@x.test, b@x.test',
    });
    expect(
      text('system', {
        notice: 'invite_request_declined',
        sender_name: 'Ada',
        invite_emails: emails,
        note: 'Not yet',
      }),
    ).toEqual({
      title: 'Ada declined your invite request',
      body: 'For a@x.test, b@x.test. Their note: Not yet',
    });
    expect(text('system', { notice: 'invite_request_declined', invite_emails: emails }).body).toBe(
      'For a@x.test, b@x.test',
    );
    expect(text('system', { notice: 'invite_request_expired' }, 'a@x.test')).toEqual({
      title: 'Your invite request expired',
      body: 'No admin answered it within 30 days. For a@x.test',
    });
  });
});

describe('notificationText in Japanese', () => {
  beforeEach(async () => {
    await i18n.changeLanguage('ja');
  });

  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('writes a message notification from who sent it and where', () => {
    expect(text('mention', sent)).toEqual({ title: 'あなたがメンションされました', body: 'hello' });
    expect(text('reply', sent).title).toBe('スレッドに新しい返信があります');
    expect(text('dm', sent).title).toBe('Ana');
    expect(text('message', sent).title).toBe('Ana さん（#general）');
    expect(text('dm').title).toBe('誰か');
    expect(text('keyword', { keyword: 'deploy', channel_name: 'ops' }).title).toBe(
      '#ops で「deploy」に一致しました',
    );
    expect(text('keyword', { keyword: 'deploy' }).title).toBe('「deploy」に一致しました');
  });

  it('writes a list item, a call and a reminder', () => {
    const item: NotificationData = { sender_name: 'Ana', item_title: 'Fix login', list_title: 'Launch plan' };
    expect(text('list', { ...item, assigned: true }, null)).toEqual({
      title: 'Ana さんが、あなたを「Fix login」の担当にしました',
      body: 'リスト: Launch plan',
    });
    expect(text('list', item).title).toBe('Ana さんが「Fix login」であなたをメンションしました');
    expect(text('list', { sender_name: 'Ana' }).title).toBe('Ana さんが「無題」であなたをメンションしました');
    expect(text('call', {}, null)).toEqual({ title: '通話の着信', body: '誰かが通話を始めようとしています' });
    expect(text('reminder', {}, null)).toEqual({
      title: 'リマインダー',
      body: '「後で」に保存したメッセージ',
    });
  });

  it.each([
    ['inactive', '作成した人のアカウントが停止された'],
    ['left_workspace', '作成した人がワークスペースから退出した'],
    ['lost_channel', '作成した人がそのチャンネルを見られなくなった'],
    ['demoted', '作成した人に、その権限がなくなった'],
  ] as const)('says a workflow stopped because %s', (stopped, reason) => {
    expect(text('system', { notice: 'workflow_stopped', workflow_name: 'Pager', stopped }, null)).toEqual({
      title: 'ワークフローがオフになりました',
      body: `「Pager」は、${reason}ため停止しました。`,
    });
  });

  it('says which other system notice it is', () => {
    expect(
      text('system', { notice: 'workflow_app_gone', workflow_name: 'Triage', app_name: 'Ticketing' }, null),
    ).toEqual({
      title: 'ワークフローのステップを実行できません',
      body: '「Triage」は Ticketing のステップを使っていますが、このアプリはもう使えません。そのステップを置き換えるまで、実行はそのステップで失敗します。',
    });
    expect(text('system', { notice: 'app_requested', sender_name: 'Bo' }, 'PagerDuty').title).toBe(
      'Bo さんがアプリをリクエストしました',
    );
    expect(text('system', { notice: 'not_delivered', failure: 'thread_gone' }, null)).toEqual({
      title: '予約したメッセージが送信されませんでした',
      body: '返信先のスレッドがなくなりました。',
    });
    const until = '2026-11-16T12:00:00Z';
    expect(text('system', { notice: 'guest_ending', guest_name: 'Vera', guest_until: until }, 'raw')).toEqual(
      {
        title: 'Vera さんのゲストアクセスがまもなく終了します',
        body: `${formatDay(until)} にアクセスが終了します。日付の変更や削除は、ワークスペースの設定 → メンバーで行えます。`,
      },
    );
    expect(text('system', {}, 'raw')).toEqual({ title: 'お知らせ', body: 'raw' });
  });

  it('tells admins of an invite request and the requester of its answer, listing the emails', () => {
    const emails = ['a@x.test', 'b@x.test'];
    expect(text('system', { notice: 'invite_requested', sender_name: 'Rita' }, 'a@x.test').title).toBe(
      'Rita さんが招待をリクエストしました',
    );
    expect(
      text('system', { notice: 'invite_request_approved', sender_name: 'Ada', invite_emails: emails }),
    ).toEqual({
      title: 'Ada さんが、あなたの招待のリクエストを承認しました',
      body: '対象: a@x.test、b@x.test',
    });
    expect(
      text('system', {
        notice: 'invite_request_declined',
        sender_name: 'Ada',
        invite_emails: emails,
        note: 'Not yet',
      }),
    ).toEqual({
      title: 'Ada さんが、あなたの招待のリクエストを却下しました',
      body: '対象: a@x.test、b@x.test。管理者からのメモ: Not yet',
    });
    expect(text('system', { notice: 'invite_request_expired' }, 'a@x.test')).toEqual({
      title: 'あなたの招待のリクエストは期限切れになりました',
      body: '30日以内に管理者が対応しませんでした。対象: a@x.test',
    });
  });

  it('keeps hostile names, titles and notes as they are', () => {
    expect(text('message', { sender_name: hostile, channel_name: hostile }).title).toBe(
      `${hostile} さん（#${hostile}）`,
    );
    expect(text('keyword', { keyword: hostile }).title).toBe(`「${hostile}」に一致しました`);
    expect(
      text('list', { sender_name: hostile, item_title: hostile, list_title: hostile, assigned: true }, null),
    ).toEqual({
      title: `${hostile} さんが、あなたを「${hostile}」の担当にしました`,
      body: `リスト: ${hostile}`,
    });
    expect(
      text('system', { notice: 'workflow_stopped', workflow_name: hostile, stopped: 'inactive' }, null).body,
    ).toBe(`「${hostile}」は、作成した人のアカウントが停止されたため停止しました。`);
    expect(
      text('system', { notice: 'workflow_app_gone', workflow_name: hostile, app_name: 'Ticketing' }, null)
        .body,
    ).toBe(
      `「${hostile}」は Ticketing のステップを使っていますが、このアプリはもう使えません。そのステップを置き換えるまで、実行はそのステップで失敗します。`,
    );
    expect(
      text('system', { notice: 'workflow_app_gone', workflow_name: 'Triage', app_name: hostile }, null).body,
    ).toBe(
      `「Triage」は ${hostile} のステップを使っていますが、このアプリはもう使えません。そのステップを置き換えるまで、実行はそのステップで失敗します。`,
    );
    expect(
      text('system', {
        notice: 'invite_request_declined',
        sender_name: hostile,
        invite_emails: [hostile],
        note: hostile,
      }).body,
    ).toBe(`対象: ${hostile}。管理者からのメモ: ${hostile}`);
  });

  it('has Japanese for every one', () => {
    function leaves(value: unknown, prefix = ''): string[] {
      if (typeof value !== 'object' || value === null) return [prefix];
      return Object.entries(value).flatMap(([key, child]) =>
        leaves(child, prefix ? `${prefix}.${key}` : key),
      );
    }
    const english = leaves(i18n.getResource('en', 'notifications', 'activity'));
    const japanese = leaves(i18n.getResource('ja', 'notifications', 'activity'));
    expect(english.length).toBeGreaterThan(0);
    expect(japanese).toEqual(english);
  });
});
