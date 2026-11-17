import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import i18n from '@/lib/i18n';
import { micErrorLabel } from './media';

describe('micErrorLabel in Japanese', () => {
  beforeEach(async () => {
    await i18n.changeLanguage('ja');
  });
  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('says why the microphone could not be opened', () => {
    expect(micErrorLabel(new DOMException('', 'NotAllowedError'))).toBe(
      'ブラウザがマイクをブロックしています。このサイトでマイクの使用を許可してから、もう一度お試しください。',
    );
    expect(micErrorLabel(new DOMException('', 'NotFoundError'))).toBe(
      'マイクが見つかりません。マイクを接続するか、設定で別の入力を選んでください。',
    );
    expect(micErrorLabel(new DOMException('', 'NotReadableError'))).toBe(
      'ほかのアプリがマイクを使用中です。そのアプリを閉じてから、もう一度お試しください。',
    );
    expect(micErrorLabel(new Error('other'))).toBe('マイクを使用できませんでした。');
  });
});
