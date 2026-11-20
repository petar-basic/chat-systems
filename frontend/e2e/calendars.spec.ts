import { test, expect } from '@playwright/test';
import { API, devWorkspace, login, userContext } from './helpers';

test('a link opens Preferences at Calendar, and Connect sends the person to Google for calendar access only', async ({
  page,
}) => {
  const { ctx: admin } = await userContext('admin@dev.local');
  const workspace = await devWorkspace(admin);
  const clientId = `calendar-e2e-${Date.now()}.apps.googleusercontent.com`;
  const app = `${API}/workspaces/${workspace.id}/apps/google_calendar`;

  try {
    const configured = await admin.put(app, {
      data: { client_id: clientId, client_secret: 'e2e-not-a-secret', enabled: true, reuse_from: null },
    });
    expect(configured.status(), await configured.text()).toBe(200);

    let authorize: URL | undefined;
    await page.route('https://accounts.google.com/**', async (route) => {
      authorize = new URL(route.request().url());
      await route.fulfill({ contentType: 'text/html', body: '<p>Google consent</p>' });
    });

    await login(page, 'alice@dev.local');
    await page.goto(`/app/${workspace.id}?preferences=calendar`);

    await expect(page.getByRole('tab', { name: 'Calendar' })).toHaveAttribute('aria-selected', 'true', {
      timeout: 20_000,
    });
    await expect(page).not.toHaveURL(/preferences/);
    const outlook = page.getByTestId('calendar-outlook_calendar');
    await expect(outlook).toContainText('Ask a workspace admin to set this up in Apps.');
    await expect(outlook.getByRole('button', { name: 'Connect' })).toHaveCount(0);

    const google = page.getByTestId('calendar-google_calendar');
    await expect(google).toContainText('Not connected');
    await google.getByRole('button', { name: 'Connect' }).click();

    await expect(page.getByText('Google consent')).toBeVisible({ timeout: 15_000 });
    expect(authorize?.pathname).toBe('/o/oauth2/v2/auth');
    expect(authorize?.searchParams.get('client_id')).toBe(clientId);
    expect(authorize?.searchParams.get('scope')).toBe(
      'https://www.googleapis.com/auth/calendar.events.readonly',
    );
    expect(authorize?.searchParams.get('code_challenge_method')).toBe('S256');
    expect(authorize?.searchParams.get('state')).toBeTruthy();
  } finally {
    await admin.delete(app);
  }
});
