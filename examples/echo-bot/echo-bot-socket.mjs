const api = (process.env.CHAT_API_URL ?? 'http://localhost:3000/api').replace(/\/$/, '');
const botToken = process.env.CHAT_BOT_TOKEN;
const appToken = process.env.CHAT_APP_TOKEN;
const once = process.argv.includes('--once');

if (!botToken || !appToken) {
  console.error('Set CHAT_BOT_TOKEN to the bot token and CHAT_APP_TOKEN to the app-level token.');
  process.exit(1);
}

async function call(token, method, path, body) {
  const response = await fetch(`${api}${path}`, {
    method,
    headers: {
      Authorization: `Bearer ${token}`,
      'Content-Type': 'application/json',
    },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  const payload = await response.json().catch(() => null);
  if (!response.ok) {
    throw new Error(`${method} ${path}: ${response.status} ${payload?.error ?? ''}`);
  }
  return payload;
}

const withoutMentions = (text) => text.replace(/@\[[^\]]*\]\([^)]*\)/g, '').trim();

async function answerMention(event) {
  const text = withoutMentions(event.text ?? '');
  if (!text) return false;
  await call(botToken, 'POST', `/channels/${event.channel_id}/messages`, {
    content: text,
    thread_parent_id: event.thread_parent_id ?? event.message_id,
    client_message_id: crypto.randomUUID(),
  });
  return true;
}

async function answerCommand(command) {
  await fetch(command.response_url, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      response_type: 'ephemeral',
      text: command.text || `/${command.command} heard you`,
    }),
  });
}

async function handle(frame) {
  if (frame.type === 'event' && frame.payload?.type === 'app_mention') {
    return answerMention(frame.payload.event);
  }
  if (frame.type === 'slash_command') {
    await answerCommand(frame.payload);
  }
  return false;
}

function connect(url) {
  return new Promise((resolve) => {
    const socket = new WebSocket(url);
    socket.addEventListener('message', async (message) => {
      const frame = JSON.parse(message.data);
      if (frame.type === 'hello') {
        console.log(`connected as app ${frame.app_id}`);
        return;
      }
      if (frame.type === 'disconnect') {
        console.log(`disconnected: ${frame.reason}`);
        return;
      }
      if (!frame.envelope_id) return;
      socket.send(JSON.stringify({ envelope_id: frame.envelope_id }));
      try {
        if ((await handle(frame)) && once) {
          console.log('answered 1');
          socket.close();
        }
      } catch (error) {
        console.error(error.message);
      }
    });
    socket.addEventListener('close', () => resolve());
    socket.addEventListener('error', () => resolve());
  });
}

const me = await call(botToken, 'GET', '/bot-apps/me');
console.log(`${me.name} is listening as ${me.bot_user_id}`);

for (;;) {
  const { url } = await call(appToken, 'POST', '/apps/connections.open');
  await connect(url);
  if (once) break;
  await new Promise((resolve) => setTimeout(resolve, 2000));
}
