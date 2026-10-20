import { test, expect, type Browser, type Page } from '@playwright/test';
import { API, devWorkspace, login, userContext } from './helpers';

const ALICE_TYPING = /Alice Johnson is typing/i;

async function dmBetweenAliceAndCharlie(browser: Browser) {
  const { ctx: alice } = await userContext('alice@dev.local');
  const { ctx: charlie, userId: charlieId } = await userContext('charlie@dev.local');
  const workspace = await devWorkspace(alice);
  const created = await alice.post(`${API}/workspaces/${workspace.id}/conversations`, {
    data: { participant_ids: [charlieId] },
  });
  expect(created.status(), 'open the direct conversation').toBe(200);
  const conversation = await created.json();
  await alice.dispose();
  await charlie.dispose();

  const contexts = [await browser.newContext(), await browser.newContext()];
  const pages: Page[] = [await contexts[0].newPage(), await contexts[1].newPage()];
  await login(pages[0], 'alice@dev.local');
  await login(pages[1], 'charlie@dev.local');
  const url = `/app/${workspace.id}/c/${conversation.id}`;
  await pages[0].goto(url);
  await pages[1].goto(url);
  await expect(pages[1].locator('[data-qa="conversation-title"]')).toBeVisible({ timeout: 20_000 });

  const composer = pages[0].locator('.ProseMirror[contenteditable="true"]').last();
  await composer.click();

  return {
    writer: pages[0],
    reader: pages[1],
    composer,
    close: async () => {
      await contexts[0].close();
      await contexts[1].close();
    },
  };
}

test('typing in a direct message reaches the other participant', async ({ browser }) => {
  const dm = await dmBetweenAliceAndCharlie(browser);
  try {
    await dm.composer.type('is this thing on', { delay: 40 });
    await expect(dm.reader.getByText(ALICE_TYPING)).toBeVisible({ timeout: 10_000 });
  } finally {
    await dm.close();
  }
});

// The indicator used to be published once and expire five seconds later, so it
// vanished from under anybody still writing. Nothing shorter than the expiry
// can catch that.
test('the typing indicator survives a message that takes a while to write', async ({ browser }) => {
  test.slow();
  const dm = await dmBetweenAliceAndCharlie(browser);
  try {
    await dm.composer.type('writing out a long thought that takes a while', { delay: 90 });
    await expect(dm.reader.getByText(ALICE_TYPING)).toBeVisible({ timeout: 10_000 });

    await dm.composer.type(' and then carrying right on with the rest of it', { delay: 90 });
    await expect(
      dm.reader.getByText(ALICE_TYPING),
      'still typing after eight seconds at the keyboard',
    ).toBeVisible();
  } finally {
    await dm.close();
  }
});
