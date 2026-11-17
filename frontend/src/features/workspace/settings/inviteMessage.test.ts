import { afterEach, describe, it, expect } from 'vitest';
import i18n from '@/lib/i18n';
import { inviteMessage } from './inviteMessage';

describe('inviteMessage', () => {
  it('words each outcome of the invite email', () => {
    expect(inviteMessage('sent', 'ana@firma.com')).toBe(
      'Invite sent to ana@firma.com. You can also send them the link yourself.',
    );
    expect(inviteMessage('failed', 'ana@firma.com')).toBe(
      'The invite email to ana@firma.com could not be sent. Send them this link yourself.',
    );
    expect(inviteMessage('not_configured', 'ana@firma.com')).toBe(
      'Email is not set up on this instance, so nothing was sent. Send ana@firma.com this link yourself.',
    );
  });
});

describe('inviteMessage in Japanese', () => {
  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('words each outcome, with the address as it was typed', async () => {
    await i18n.changeLanguage('ja');
    const email = '<img src=x onerror="alert(1)">{{email}}@firma.com';
    expect(inviteMessage('sent', email)).toBe(
      `${email} に招待を送りました。リンクを自分で送ることもできます。`,
    );
    expect(inviteMessage('failed', email)).toBe(
      `${email} への招待メールを送れませんでした。このリンクを自分で送ってください。`,
    );
    expect(inviteMessage('not_configured', email)).toBe(
      `このサーバーではメールが設定されていないため、何も送られていません。このリンクを ${email} に自分で送ってください。`,
    );
  });
});
