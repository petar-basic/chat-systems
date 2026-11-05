type Json = Record<string, unknown>;

interface CredentialDescriptorJson {
  type: PublicKeyCredentialType;
  id: string;
  transports?: AuthenticatorTransport[];
}

export function passkeysSupported(): boolean {
  return typeof window !== 'undefined' && typeof window.PublicKeyCredential === 'function';
}

export function deviceName(userAgent: string = navigator.userAgent): string {
  if (/iPhone/.test(userAgent)) return 'iPhone';
  if (/iPad/.test(userAgent)) return 'iPad';
  if (/Android/.test(userAgent)) return 'Android';
  if (/Mac OS X|Macintosh/.test(userAgent)) return 'Mac';
  if (/Windows/.test(userAgent)) return 'Windows';
  if (/Linux/.test(userAgent)) return 'Linux';
  return 'This device';
}

export function fromBase64Url(value: string): ArrayBuffer {
  const base64 = value.replace(/-/g, '+').replace(/_/g, '/');
  const padded = base64 + '='.repeat((4 - (base64.length % 4)) % 4);
  const binary = atob(padded);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i += 1) bytes[i] = binary.charCodeAt(i);
  return bytes.buffer;
}

export function toBase64Url(buffer: ArrayBuffer): string {
  const bytes = new Uint8Array(buffer);
  let binary = '';
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
}

function descriptors(list: unknown): PublicKeyCredentialDescriptor[] | undefined {
  if (!Array.isArray(list)) return undefined;
  return (list as CredentialDescriptorJson[]).map((item) => ({ ...item, id: fromBase64Url(item.id) }));
}

function publicKeyOf(options: unknown): Json {
  const publicKey = (options as { publicKey?: Json } | null)?.publicKey;
  if (!publicKey) throw new Error('The server sent no passkey request');
  return publicKey;
}

export function creationOptions(options: unknown): PublicKeyCredentialCreationOptions {
  const publicKey = publicKeyOf(options);
  const user = publicKey.user as Json;
  return {
    ...(publicKey as unknown as PublicKeyCredentialCreationOptions),
    challenge: fromBase64Url(publicKey.challenge as string),
    user: { ...(user as unknown as PublicKeyCredentialUserEntity), id: fromBase64Url(user.id as string) },
    excludeCredentials: descriptors(publicKey.excludeCredentials),
  };
}

export function requestOptions(options: unknown): PublicKeyCredentialRequestOptions {
  const publicKey = publicKeyOf(options);
  return {
    ...(publicKey as unknown as PublicKeyCredentialRequestOptions),
    challenge: fromBase64Url(publicKey.challenge as string),
    allowCredentials: descriptors(publicKey.allowCredentials),
  };
}

export function registrationJson(credential: PublicKeyCredential): Json {
  const response = credential.response as AuthenticatorAttestationResponse;
  return {
    id: credential.id,
    rawId: toBase64Url(credential.rawId),
    type: credential.type,
    response: {
      clientDataJSON: toBase64Url(response.clientDataJSON),
      attestationObject: toBase64Url(response.attestationObject),
      transports: response.getTransports?.() ?? [],
    },
    extensions: credential.getClientExtensionResults(),
  };
}

export function assertionJson(credential: PublicKeyCredential): Json {
  const response = credential.response as AuthenticatorAssertionResponse;
  return {
    id: credential.id,
    rawId: toBase64Url(credential.rawId),
    type: credential.type,
    response: {
      clientDataJSON: toBase64Url(response.clientDataJSON),
      authenticatorData: toBase64Url(response.authenticatorData),
      signature: toBase64Url(response.signature),
      userHandle: response.userHandle ? toBase64Url(response.userHandle) : null,
    },
    extensions: credential.getClientExtensionResults(),
  };
}

export async function createPasskey(options: unknown): Promise<Json> {
  const credential = await navigator.credentials.create({ publicKey: creationOptions(options) });
  if (!credential) throw new Error('No passkey was created');
  return registrationJson(credential as PublicKeyCredential);
}

export async function requestPasskey(options: unknown): Promise<Json> {
  const credential = await navigator.credentials.get({ publicKey: requestOptions(options) });
  if (!credential) throw new Error('No passkey was chosen');
  return assertionJson(credential as PublicKeyCredential);
}

export function wasCancelled(error: unknown): boolean {
  return error instanceof DOMException && (error.name === 'NotAllowedError' || error.name === 'AbortError');
}
