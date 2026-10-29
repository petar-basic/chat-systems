const api = (process.env.CHAT_API_URL ?? 'http://localhost:3000/api').replace(/\/$/, '');
const token = process.env.CHAT_BOT_TOKEN;
const once = process.argv.includes('--once');
const pollMs = 3000;

if (!token) {
  console.error('Set CHAT_BOT_TOKEN to the token shown when the app was made.');
  process.exit(1);
}

async function call(method, path, body) {
  const response = await fetch(`${api}${path}`, {
    method,
    headers: { Authorization: `Bearer ${token}`, 'Content-Type': 'application/json' },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  if (response.status === 429) {
    const wait = Number(response.headers.get('retry-after') ?? '60');
    await new Promise((resolve) => setTimeout(resolve, wait * 1000));
    return call(method, path, body);
  }
  const payload = await response.json().catch(() => null);
  if (!response.ok) {
    throw new Error(`${method} ${path}: ${response.status} ${payload?.error ?? ''}`);
  }
  return payload;
}

function echoOf(message, me) {
  if (message.user_id === me || message.thread_parent_id || message.reply_count > 0) return null;
  const match = /^echo\s+(.+)/is.exec(message.content.trim());
  return match ? match[1] : null;
}

async function pass(me, workspaceId, since) {
  const channels = (await call('GET', `/workspaces/${workspaceId}/channels`)).data;
  let answered = 0;
  for (const channel of channels) {
    const messages = (await call('GET', `/channels/${channel.id}/messages?limit=50`)).data;
    for (const message of messages) {
      if (new Date(message.created_at) < since) continue;
      const text = echoOf(message, me);
      if (text === null) continue;
      await call('POST', `/channels/${channel.id}/messages`, {
        content: text,
        thread_parent_id: message.id,
        client_message_id: crypto.randomUUID(),
      });
      answered += 1;
    }
  }
  return answered;
}

const app = await call('GET', '/bot-apps/me');
console.log(`${app.name} is listening as ${app.bot_user_id}`);

if (once) {
  const answered = await pass(app.bot_user_id, app.workspace_id, new Date(0));
  console.log(`answered ${answered}`);
} else {
  let since = new Date();
  for (;;) {
    const started = new Date();
    try {
      await pass(app.bot_user_id, app.workspace_id, since);
      since = started;
    } catch (error) {
      console.error(error.message);
    }
    await new Promise((resolve) => setTimeout(resolve, pollMs));
  }
}
