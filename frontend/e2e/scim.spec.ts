import { test, expect, type APIRequestContext } from '@playwright/test';
import { API, MAILHOG, PASSWORD, authHeaders } from './helpers';

async function scimToken(admin: APIRequestContext) {
  const res = await admin.post(`${API}/admin/scim/tokens`, {
    data: { description: `e2e-${Date.now()}` },
  });
  expect(res.status(), 'minting a SCIM token').toBe(200);
  return (await res.json()).token as string;
}

async function inviteAndRegister(admin: APIRequestContext, request: APIRequestContext, email: string) {
  const workspaces = await (await admin.get(`${API}/workspaces`)).json();
  const workspaceId = workspaces.data[0].id as string;

  const invite = await admin.post(`${API}/workspaces/${workspaceId}/invites`, {
    data: { email, role: 'member' },
  });
  expect(invite.status(), 'inviting the person we are about to deprovision').toBe(200);

  const mail = await request.get(`${MAILHOG}/api/v2/search?kind=to&query=${encodeURIComponent(email)}`);
  const items = (await mail.json()).items;
  expect(items.length, 'the invite email arrived').toBeGreaterThan(0);
  const body: string = items[0].Content.Body.replace(/=\r\n/g, '').replace(/=3D/g, '=');
  const token = body.match(/invite\/([A-Za-z0-9_.-]+)/)?.[1];
  expect(token, 'the invite link carries a registration token').toBeTruthy();

  const accepted = await request.post(`${API}/auth/complete-registration`, {
    data: { token, password: PASSWORD, display_name: 'Leaver' },
  });
  expect(accepted.status(), 'accepting the invite').toBe(200);
  const session = await accepted.json();
  return {
    workspaceId,
    userId: session.user.id as string,
    accessToken: session.access_token as string,
  };
}

/**
 * The point of the ticket end to end: one call from the identity provider has to
 * leave nothing behind. Everything below the endpoint already existed — this asserts
 * the composition, which is the part that silently degrades if a step is dropped.
 */
test('deprovisioning through SCIM ends the session and the memberships', async ({ playwright }) => {
  const admin = await playwright.request.newContext({
    extraHTTPHeaders: await authHeaders(await playwright.request.newContext(), 'admin@dev.local'),
  });
  const anonymous = await playwright.request.newContext();

  const email = `leaver-${Date.now()}@dev.local`;
  const { workspaceId, userId, accessToken } = await inviteAndRegister(admin, anonymous, email);

  const leaver = await playwright.request.newContext({
    extraHTTPHeaders: { Authorization: `Bearer ${accessToken}` },
  });

  try {
    const before = await leaver.get(`${API}/workspaces`);
    expect(before.status()).toBe(200);
    expect((await before.json()).data.map((w: { id: string }) => w.id)).toContain(workspaceId);

    const token = await scimToken(admin);
    const provider = await playwright.request.newContext({
      extraHTTPHeaders: { Authorization: `Bearer ${token}` },
    });

    const patched = await provider.patch(`${API}/scim/v2/Users/${userId}`, {
      data: {
        schemas: ['urn:ietf:params:scim:api:messages:2.0:PatchOp'],
        Operations: [{ op: 'replace', path: 'active', value: false }],
      },
    });
    expect(patched.status(), await patched.text()).toBe(200);
    expect((await patched.json()).active).toBe(false);

    const after = await leaver.get(`${API}/workspaces`);
    expect(after.status(), 'the token they were holding stops working').toBe(401);

    const denied = await anonymous.post(`${API}/auth/login`, {
      data: { email, password: PASSWORD },
    });
    expect(denied.status(), 'and they cannot sign in again').toBe(401);

    const reactivated = await provider.patch(`${API}/scim/v2/Users/${userId}`, {
      data: {
        schemas: ['urn:ietf:params:scim:api:messages:2.0:PatchOp'],
        Operations: [{ op: 'replace', path: 'active', value: true }],
      },
    });
    expect(reactivated.status()).toBe(200);

    const back = await anonymous.post(`${API}/auth/login`, {
      data: { email, password: PASSWORD },
    });
    expect(back.status(), 'the account comes back').toBe(200);

    const restored = await playwright.request.newContext({
      extraHTTPHeaders: { Authorization: `Bearer ${(await back.json()).access_token}` },
    });
    const workspaces = await (await restored.get(`${API}/workspaces`)).json();
    expect(
      workspaces.data.map((w: { id: string }) => w.id),
      'the access does not: coming back needs a fresh invite',
    ).not.toContain(workspaceId);
    await restored.dispose();
    await provider.dispose();
  } finally {
    await admin.dispose();
    await anonymous.dispose();
    await leaver.dispose();
  }
});

test('the SCIM surface is closed to a session token', async ({ playwright }) => {
  const admin = await playwright.request.newContext({
    extraHTTPHeaders: await authHeaders(await playwright.request.newContext(), 'admin@dev.local'),
  });
  try {
    const res = await admin.get(`${API}/scim/v2/Users`);
    expect(res.status(), 'an admin session is not a provisioning credential').toBe(401);
    expect((await res.json()).schemas[0]).toBe('urn:ietf:params:scim:api:messages:2.0:Error');
  } finally {
    await admin.dispose();
  }
});
