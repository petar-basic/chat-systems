import { test, expect } from '@playwright/test';
import { execFileSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { API, login, userContext, devWorkspace } from './helpers';

test.setTimeout(90_000);

// A minimal export, written and zipped the way Slack hands one over.
function writeExport(stamp: number): string {
  const channel = `acme-${stamp}`;
  // A Slack id of its own as well as a name: the mapping table remembers that
  // channel C1 was already imported here, which is exactly what stops a re-run
  // from duplicating — and would make two fixtures look like the same channel.
  const channelId = `C${stamp}`;
  const root = mkdtempSync(join(tmpdir(), 'slack-export-'));
  mkdirSync(join(root, channel));

  writeFileSync(
    join(root, 'users.json'),
    JSON.stringify([
      { id: 'U1', name: 'admin', profile: { email: 'admin@dev.local', real_name: 'Admin' } },
      { id: 'U2', name: 'bob', profile: { email: 'bob@dev.local', real_name: 'Bob Smith' } },
    ]),
  );
  writeFileSync(
    join(root, 'channels.json'),
    JSON.stringify([
      {
        id: channelId,
        name: channel,
        members: ['U1', 'U2'],
        topic: { value: '' },
        purpose: { value: '' },
      },
    ]),
  );
  writeFileSync(
    join(root, channel, '2024-03-01.json'),
    JSON.stringify([
      { type: 'message', user: 'U1', text: `*imported* ${stamp}`, ts: '1709251200.000100' },
      {
        type: 'message',
        user: 'U2',
        text: 'and a reply',
        ts: '1709251300.000100',
        thread_ts: '1709251200.000100',
      },
    ]),
  );

  // Named after the run, so the panel's list — which keeps what earlier runs
  // left behind — can be read without counting rows.
  const archive = join(tmpdir(), `acme-${stamp}.zip`);
  execFileSync('zip', ['-rq', archive, '.'], { cwd: root });
  rmSync(root, { recursive: true, force: true });
  return archive;
}

test('an admin imports a Slack export from the app and watches it finish', async ({ page }) => {
  const { ctx: admin } = await userContext('admin@dev.local');
  const workspace = await devWorkspace(admin);
  const stamp = Date.now();
  const archive = writeExport(stamp);

  try {
    await login(page, 'admin@dev.local');
    await page.goto(`/app/${workspace.id}`);

    await page.getByRole('button', { name: workspace.name }).click();
    await page.locator('[data-qa="open-slack-import"]').click();

    const panel = page.locator('[data-qa="slack-import-panel"]');
    await expect(panel).toBeVisible();
    await expect(panel.locator('[data-qa="slack-import-dry-run"]')).toBeChecked();

    // A dry run first, which is what the panel nudges you towards.
    await panel.locator('[data-qa="slack-import-file"]').setInputFiles(archive);
    await panel.locator('[data-qa="slack-import-start"]').click();

    const firstRun = panel.locator('[data-qa="slack-import-run"]').first();
    await expect(firstRun.locator('[data-qa="slack-import-source"]')).toHaveText(`acme-${stamp}.zip`);
    await expect(firstRun.locator('[data-qa="slack-import-status"]')).toHaveText('complete', {
      timeout: 30_000,
    });
    await expect(firstRun).toContainText('dry run');
    await expect(firstRun).toContainText('2 messages');

    const afterDryRun = await (await admin.get(`${API}/workspaces/${workspace.id}/channels`)).json();
    expect(
      afterDryRun.data.some((c: { name: string }) => c.name === `acme-${stamp}`),
      'a dry run writes nothing',
    ).toBe(false);

    // Then the real one.
    await panel.locator('[data-qa="slack-import-dry-run"]').uncheck();
    await panel.locator('[data-qa="slack-import-file"]').setInputFiles(archive);
    await panel.locator('[data-qa="slack-import-start"]').click();

    const realRun = panel.locator('[data-qa="slack-import-run"]').first();
    await expect(realRun, 'the newest row is the real run now').not.toContainText('dry run');
    await expect(realRun.locator('[data-qa="slack-import-status"]')).toHaveText('complete', {
      timeout: 30_000,
    });
    await expect(realRun).toContainText('1 channels');

    const channels = await (await admin.get(`${API}/workspaces/${workspace.id}/channels`)).json();
    const imported = channels.data.find((c: { name: string }) => c.name === `acme-${stamp}`);
    expect(imported, 'the channel is there now').toBeTruthy();

    const messages = await (await admin.get(`${API}/channels/${imported.id}/messages`)).json();
    const root = messages.data.find((m: { content: string }) => m.content.includes(`${stamp}`));
    expect(root.content, 'mrkdwn became markdown').toContain(`**imported** ${stamp}`);
    expect(root.reply_count, 'and the thread came with it').toBe(1);
    expect(new Date(root.created_at).getUTCFullYear(), 'imported history keeps the date it was written').toBe(
      2024,
    );
  } finally {
    rmSync(archive, { force: true });
    await admin.dispose();
  }
});

test('an export can bring its own workspace, named from the file', async ({ page }) => {
  const { ctx: admin } = await userContext('admin@dev.local');
  const workspace = await devWorkspace(admin);
  const stamp = Date.now();
  const archive = writeExport(stamp);

  try {
    await login(page, 'admin@dev.local');
    await page.goto(`/app/${workspace.id}`);
    await page.getByRole('button', { name: workspace.name }).click();
    await page.locator('[data-qa="open-slack-import"]').click();

    const panel = page.locator('[data-qa="slack-import-panel"]');
    await panel.locator('[data-qa="slack-import-file"]').setInputFiles(archive);
    await panel.locator('[data-qa="slack-import-into-new"]').check();

    // The name is suggested from the file, which is the only place Slack writes
    // it — the archive itself does not carry it.
    const name = panel.locator('[data-qa="slack-import-workspace-name"]');
    await expect(name).toHaveValue(`acme-${stamp}`);
    await name.fill(`Acme ${stamp}`);

    await panel.locator('[data-qa="slack-import-dry-run"]').uncheck();
    await panel.locator('[data-qa="slack-import-start"]').click();

    await expect
      .poll(
        async () => {
          const list = await (await admin.get(`${API}/workspaces`)).json();
          return list.data.some((w: { name: string }) => w.name === `Acme ${stamp}`);
        },
        { timeout: 30_000 },
      )
      .toBe(true);

    const workspaces = await (await admin.get(`${API}/workspaces`)).json();
    const created = workspaces.data.find((w: { name: string }) => w.name === `Acme ${stamp}`);
    await expect
      .poll(
        async () => {
          const channels = await (await admin.get(`${API}/workspaces/${created.id}/channels`)).json();
          return channels.data.some((c: { name: string }) => c.name === `acme-${stamp}`);
        },
        { timeout: 30_000 },
      )
      .toBe(true);
  } finally {
    rmSync(archive, { force: true });
    await admin.dispose();
  }
});

test('a plain member is not offered the import at all', async ({ page }) => {
  const { ctx: bob } = await userContext('bob@dev.local');
  const workspace = await devWorkspace(bob);

  try {
    await login(page, 'bob@dev.local');
    await page.goto(`/app/${workspace.id}`);
    await page.getByRole('button', { name: workspace.name }).click();

    await expect(page.locator('[data-qa="open-slack-import"]')).toHaveCount(0);

    const refused = await bob.post(`${API}/workspaces/${workspace.id}/slack-imports`, {
      multipart: {
        dry_run: 'true',
        archive: { name: 'x.zip', mimeType: 'application/zip', buffer: Buffer.from('PK') },
      },
    });
    expect(refused.status(), 'and the endpoint says no as well').toBe(403);
  } finally {
    await bob.dispose();
  }
});
