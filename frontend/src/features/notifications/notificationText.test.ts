import { afterEach, describe, expect, it } from 'vitest';
import i18n from '@/lib/i18n';
import { notificationText, type NotificationData } from './notificationText';

const t = i18n.getFixedT(null, ['notifications', 'messages'] as const);
const text = (kind: string, data: NotificationData = {}, body: string | null = 'hello') =>
  notificationText(t, kind, data, body);

const sent: NotificationData = { sender_name: 'Ana', channel_name: 'general' };

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
  });

  it('follows the reader’s language, and falls back to English for a string not yet translated', async () => {
    i18n.addResource('ja', 'notifications', 'activity.reply', 'スレッドに新しい返信');
    await i18n.changeLanguage('ja');
    expect(text('reply').title).toBe('スレッドに新しい返信');
    expect(text('mention').title).toBe('You were mentioned');
    expect(text('system', { notice: 'not_delivered', failure: 'thread_gone' }, null).body).toBe(
      '返信先のスレッドがなくなりました。',
    );
  });
});
