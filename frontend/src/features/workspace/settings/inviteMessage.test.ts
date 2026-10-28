import { describe, it, expect } from 'vitest';
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
