import { test, expect } from '@playwright/test';
import { API, login, userContext, devWorkspace, openWorkspaceSettings } from './helpers';

test.setTimeout(90_000);

test('the owner sets data retention from the dialog, with the count shown first', async ({ page }) => {
  const { ctx: owner } = await userContext('admin@dev.local');
  const workspace = await devWorkspace(owner);
  const before = (await (await owner.get(`${API}/workspaces/${workspace.id}/retention`)).json()).policy;

  try {
    await login(page, 'admin@dev.local');
    await page.goto(`/app/${workspace.id}`);
    await openWorkspaceSettings(page, 'retention');

    const section = page.locator('[data-qa="retention-section"]');
    await expect(section).toBeVisible();

    await section.locator('[data-qa="retention-messages-days"]').click();
    await section.locator('[data-qa="retention-messages-input"]').fill('3650');
    await expect(section.locator('[data-qa="retention-preview"]')).toHaveText(
      /About [\d,]+ messages? and [\d,]+ files? would be removed now\./,
    );

    await section.locator('[data-qa="retention-save"]').click();
    await expect(section.locator('[data-qa="retention-save-status"]')).toHaveText('Saved');

    const saved = (await (await owner.get(`${API}/workspaces/${workspace.id}/retention`)).json()).policy;
    expect(saved.message_days).toBe(3650);
    expect(saved.file_days, 'files were left as they were').toBe(before?.file_days ?? null);
  } finally {
    await owner.patch(`${API}/workspaces/${workspace.id}/retention`, {
      data: {
        message_days: before?.message_days ?? null,
        file_days: before?.file_days ?? null,
        audit_days: before?.audit_days,
        notification_days: before?.notification_days,
      },
    });
    await owner.dispose();
  }
});

test('the owner starts a workspace export and downloads it once', async ({ page }) => {
  const { ctx: owner } = await userContext('admin@dev.local');
  const workspace = await devWorkspace(owner);

  try {
    await login(page, 'admin@dev.local');
    await page.goto(`/app/${workspace.id}`);
    await openWorkspaceSettings(page, 'exports');

    const section = page.locator('[data-qa="exports-section"]');
    const rowsBefore = await section.locator('[data-qa="export-row"]').count();
    await section.locator('[data-qa="export-start"]').click();
    await expect(section.locator('[data-qa="export-row"]')).toHaveCount(rowsBefore + 1);

    const newest = section.locator('[data-qa="export-row"]').first();
    await expect(newest.locator('[data-qa="export-status"]')).toHaveText('complete', { timeout: 45_000 });

    const link = newest.locator('[data-qa="export-download"]');
    await expect(link).toBeVisible();
    const href = await link.getAttribute('href');
    expect(href).toContain('/api/exports/download/');

    const archive = await owner.get(href!);
    expect(archive.status()).toBe(200);
    expect(archive.headers()['content-type']).toBe('application/x-tar');

    const again = await owner.get(href!);
    expect(again.status(), 'the link works once').toBe(404);
  } finally {
    await owner.dispose();
  }
});
