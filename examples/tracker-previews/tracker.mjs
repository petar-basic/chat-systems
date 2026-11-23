const api = (process.env.CHAT_API_URL ?? "http://localhost:3000/api/v1").replace(
  /\/$/,
  "",
);
const botToken = process.env.CHAT_BOT_TOKEN;
const appToken = process.env.CHAT_APP_TOKEN;

if (!botToken || !appToken) {
  console.error(
    "Set CHAT_BOT_TOKEN to the bot token and CHAT_APP_TOKEN to the app-level token.",
  );
  process.exit(1);
}

async function call(token, method, path, body) {
  const response = await fetch(`${api}${path}`, {
    method,
    headers: {
      Authorization: `Bearer ${token}`,
      "Content-Type": "application/json",
    },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  const payload = await response.json().catch(() => null);
  if (!response.ok) {
    throw new Error(
      `${method} ${path}: ${response.status} ${payload?.error ?? ""}`,
    );
  }
  return payload;
}

const STATUSES = ["Open", "In progress", "Done"];
const PEOPLE = ["Ana", "Marko", "Jelena"];

function issueOf(url) {
  const number = Number(new URL(url).pathname.match(/\/issues\/(\d+)/)?.[1]);
  if (!Number.isInteger(number)) return null;
  return {
    key: `TRK-${number}`,
    title:
      number % 2 === 0 ? "Search misses new messages" : "Login fails on Safari",
    status: STATUSES[number % STATUSES.length],
    assignee: PEOPLE[number % PEOPLE.length],
  };
}

function preview(issue) {
  return [
    {
      type: "header",
      text: { type: "plain_text", text: `${issue.key} ${issue.title}` },
    },
    {
      type: "section",
      fields: [
        { type: "mrkdwn", text: `*Status*\n${issue.status}` },
        { type: "mrkdwn", text: `*Assignee*\n${issue.assignee}` },
      ],
    },
  ];
}

async function linkShared(event) {
  const unfurls = {};
  for (const { url } of event.links) {
    const issue = issueOf(url);
    if (issue) unfurls[url] = { blocks: preview(issue) };
  }
  const urls = Object.keys(unfurls);
  console.log(`link_shared ${event.message_id}: ${urls.join(" ")}`);
  if (urls.length === 0) return;
  await call(botToken, "POST", `/messages/${event.message_id}/unfurls`, {
    unfurls,
  });
}

function connect(url) {
  return new Promise((resolve) => {
    const socket = new WebSocket(url);
    socket.addEventListener("message", async (message) => {
      const frame = JSON.parse(message.data);
      if (frame.type === "hello") {
        console.log(`connected as app ${frame.app_id}`);
        return;
      }
      if (!frame.envelope_id) return;
      socket.send(JSON.stringify({ envelope_id: frame.envelope_id }));
      if (frame.type !== "event" || frame.payload.type !== "link_shared")
        return;
      try {
        await linkShared(frame.payload.event);
      } catch (error) {
        console.error(error.message);
      }
    });
    socket.addEventListener("close", () => resolve());
    socket.addEventListener("error", () => resolve());
  });
}

const me = await call(botToken, "GET", "/bot-apps/me");
console.log(`${me.name} is listening as ${me.bot_user_id}`);

for (;;) {
  const { url } = await call(appToken, "POST", "/apps/connections.open");
  await connect(url);
  await new Promise((resolve) => setTimeout(resolve, 2000));
}
