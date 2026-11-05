import { describe, it, expect } from 'vitest';
import {
  assertionJson,
  creationOptions,
  deviceName,
  fromBase64Url,
  requestOptions,
  toBase64Url,
} from './webauthn';

function bytes(...values: number[]) {
  return new Uint8Array(values).buffer;
}

describe('webauthn', () => {
  it('carries bytes through base64url and back without padding', () => {
    const raw = bytes(251, 255, 0, 1, 2);
    const encoded = toBase64Url(raw);

    expect(encoded).toBe('-_8AAQI');
    expect(new Uint8Array(fromBase64Url(encoded))).toEqual(new Uint8Array(raw));
  });

  it('turns the server request into what the browser takes', () => {
    const options = creationOptions({
      publicKey: {
        challenge: 'AQID',
        rp: { id: 'chat.example.com', name: 'Chat' },
        user: { id: 'BAU', name: 'ana@example.com', displayName: 'Ana' },
        pubKeyCredParams: [{ type: 'public-key', alg: -7 }],
        excludeCredentials: [{ type: 'public-key', id: 'Bgc' }],
        authenticatorSelection: { residentKey: 'required', userVerification: 'required' },
      },
    });

    expect(new Uint8Array(options.challenge as ArrayBuffer)).toEqual(new Uint8Array([1, 2, 3]));
    expect(new Uint8Array(options.user.id as ArrayBuffer)).toEqual(new Uint8Array([4, 5]));
    expect(new Uint8Array(options.excludeCredentials![0].id as ArrayBuffer)).toEqual(new Uint8Array([6, 7]));
    expect(options.authenticatorSelection?.residentKey).toBe('required');
  });

  it('asks for any passkey when the server names none', () => {
    const options = requestOptions({ publicKey: { challenge: 'AQID', userVerification: 'required' } });

    expect(options.allowCredentials).toBeUndefined();
    expect(options.userVerification).toBe('required');
  });

  it('refuses a request with nothing in it', () => {
    expect(() => requestOptions({})).toThrow();
  });

  it('sends the assertion back as base64url, with who it belongs to', () => {
    const credential = {
      id: 'Bgc',
      rawId: bytes(6, 7),
      type: 'public-key',
      response: {
        clientDataJSON: bytes(1),
        authenticatorData: bytes(2),
        signature: bytes(3),
        userHandle: bytes(4),
      },
      getClientExtensionResults: () => ({}),
    } as unknown as PublicKeyCredential;

    expect(assertionJson(credential)).toEqual({
      id: 'Bgc',
      rawId: 'Bgc',
      type: 'public-key',
      response: { clientDataJSON: 'AQ', authenticatorData: 'Ag', signature: 'Aw', userHandle: 'BA' },
      extensions: {},
    });
  });

  it('names a passkey after the device it was made on', () => {
    expect(deviceName('Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)')).toBe('Mac');
    expect(deviceName('Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X)')).toBe('iPhone');
    expect(deviceName('Mozilla/5.0 (Windows NT 10.0; Win64; x64)')).toBe('Windows');
    expect(deviceName('curl/8')).toBe('This device');
  });
});
