import { test, expect } from '@playwright/test';
import {
  API,
  PASSWORD,
  login,
  userContext,
  devWorkspace,
  openWorkspaceSettings,
  createChannel,
} from './helpers';

test('a single-channel guest is invited to one channel, accepts, and lands there alone', async ({
  page,
  browser,
}) => {
  const { ctx: admin } = await userContext('admin@dev.local');
  const workspace = await devWorkspace(admin);
  const vendor = await createChannel(admin, workspace.id, 'vendor');
  const other = await createChannel(admin, workspace.id, 'ops');
  const email = `vendor-guest-${Date.now()}@example.com`;
  let guestId: string | null = null;

  try {
    await login(page, 'admin@dev.local');
    await page.goto(`/app/${workspace.id}`);
    await openWorkspaceSettings(page, 'members');

    await page.getByRole('button', { name: 'Invite people' }).click();
    await page.getByLabel('Email address to invite').fill(email);
    await page.getByLabel('Invite role').selectOption('guest');
    await page.getByTestId('guest-invite-kind').selectOption('single');
    await expect(page.getByTestId('invite-submit'), 'a guest goes somewhere').toBeDisabled();
    await page.getByTestId(`guest-invite-channel-${vendor.name}`).check();
    await page.getByTestId(`guest-invite-channel-${other.name}`).check();
    await expect(
      page.getByTestId(`guest-invite-channel-${vendor.name}`),
      'one channel only',
    ).not.toBeChecked();
    await page.getByTestId(`guest-invite-channel-${vendor.name}`).check();
    await page.getByTestId('invite-submit').click();
    const link = (await page.getByTestId('invite-link-value').textContent())!;
    expect(link).toContain('/invite/');

    const guestContext = await browser.newContext();
    const guest = await guestContext.newPage();
    await guest.goto(new URL(link).pathname);
    await guest.getByLabel('Display name').fill('Vendor Guest');
    await guest.getByLabel('Password', { exact: true }).fill(PASSWORD);
    await guest.getByLabel('Confirm password').fill(PASSWORD);
    await guest.getByRole('button', { name: 'Create account' }).click();
    await guest.waitForURL(new RegExp(`/app/${workspace.id}`), { timeout: 20_000 });
    const nav = guest.getByRole('navigation');
    await expect(nav.getByText(vendor.name)).toBeVisible();
    await expect(nav.getByText(other.name)).toHaveCount(0);
    await expect(nav.getByText('general', { exact: true })).toHaveCount(0);
    await guestContext.close();

    const members = await (await admin.get(`${API}/workspaces/${workspace.id}/members`)).json();
    const seat = members.data.find((m: { email: string }) => m.email === email);
    guestId = seat?.user_id ?? null;
    expect(seat?.guest_kind).toBe('single');

    const refused = await admin.post(`${API}/channels/${other.id}/members`, { data: { user_id: guestId } });
    expect(refused.status(), 'a second channel waits for a multi-channel guest').toBe(409);

    await page.reload();
    await openWorkspaceSettings(page, 'members');
    const row = page.getByTestId('member-row').filter({ hasText: 'Vendor Guest' });
    await expect(row.getByTestId('member-guest-terms')).toHaveText('Single-channel guest');
  } finally {
    if (guestId) await admin.delete(`${API}/workspaces/${workspace.id}/members/${guestId}`);
    await admin.delete(`${API}/channels/${vendor.id}`);
    await admin.delete(`${API}/channels/${other.id}`);
    await admin.dispose();
  }
});
