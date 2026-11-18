const api = (process.env.CHAT_API_URL ?? "http://localhost:3000/api").replace(
  /\/$/,
  "",
);
const botToken = process.env.CHAT_BOT_TOKEN;
const appToken = process.env.CHAT_APP_TOKEN;
const once = process.argv.includes("--once");

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

const text = (value) => ({ type: "plain_text", text: value });
const requests = new Map();
let created = 0;

const requestsOf = (userId) => requests.get(userId) ?? [];

function homeView(userId) {
  const mine = requestsOf(userId);
  return {
    type: "home",
    callback_id: "helpdesk-home",
    blocks: [
      { type: "header", text: text("Your requests") },
      {
        type: "actions",
        block_id: "home",
        elements: [
          {
            type: "button",
            action_id: "new_request",
            style: "primary",
            text: text("New request"),
          },
        ],
      },
      ...(mine.length === 0
        ? [
            {
              type: "context",
              elements: [{ type: "plain_text", text: "Nothing asked yet." }],
            },
          ]
        : mine.map((request) => ({
            type: "section",
            text: {
              type: "mrkdwn",
              text: `*#${request.number}* ${request.title} · ${request.status}`,
            },
          }))),
    ],
  };
}

async function publishHome(userId) {
  await call(botToken, "POST", "/views/publish", {
    user_id: userId,
    view: homeView(userId),
  });
}

const requestView = (callbackId, title, initial, channelId) => ({
  type: "modal",
  callback_id: callbackId,
  private_metadata: channelId ?? "",
  title: text(title),
  submit: text("Send"),
  close: text("Cancel"),
  blocks: [
    {
      type: "input",
      block_id: "what",
      label: text("What do you need?"),
      element: {
        type: "plain_text_input",
        action_id: "text",
        multiline: true,
        initial_value: initial,
      },
    },
  ],
});

async function openView(triggerId, view) {
  await call(botToken, "POST", "/views/open", { trigger_id: triggerId, view });
}

function submitted(payload) {
  const view = payload.view;
  const title = (view.state.values.what.text.value ?? "").trim();
  const userId = payload.user.id;
  created += 1;
  requests.set(userId, [
    ...requestsOf(userId),
    { number: created, title, status: "Open" },
  ]);
  return {
    answer: null,
    after: async () => {
      await publishHome(userId);
      if (view.callback_id === "ticket" && view.private_metadata) {
        await call(
          botToken,
          "POST",
          `/channels/${view.private_metadata}/messages`,
          {
            content: `Ticket #${created} opened by ${payload.user.name}: ${title}`,
            client_message_id: crypto.randomUUID(),
          },
        );
      }
      return true;
    },
  };
}

function interact(payload) {
  if (payload.type === "view_submission") return submitted(payload);
  if (payload.type === "shortcut" && payload.callback_id === "new_request") {
    return {
      answer: null,
      after: () =>
        openView(
          payload.trigger_id,
          requestView("request", "New request", undefined, payload.channel?.id),
        ),
    };
  }
  if (
    payload.type === "message_action" &&
    payload.callback_id === "ticket_from_message"
  ) {
    return {
      answer: null,
      after: () =>
        openView(
          payload.trigger_id,
          requestView(
            "ticket",
            "Create a ticket",
            payload.message.text,
            payload.channel.id,
          ),
        ),
    };
  }
  if (
    payload.type === "block_actions" &&
    payload.view?.type === "home" &&
    payload.actions[0]?.action_id === "new_request"
  ) {
    return {
      answer: null,
      after: () =>
        openView(payload.trigger_id, requestView("request", "New request")),
    };
  }
  return { answer: null };
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
      try {
        if (frame.type === "event") {
          socket.send(JSON.stringify({ envelope_id: frame.envelope_id }));
          if (frame.payload.type === "app_home_opened") {
            await publishHome(frame.payload.event.user_id);
          }
          return;
        }
        if (frame.type !== "interactive") {
          socket.send(JSON.stringify({ envelope_id: frame.envelope_id }));
          return;
        }
        const { answer, after } = interact(frame.payload);
        socket.send(
          JSON.stringify({
            envelope_id: frame.envelope_id,
            payload: answer ?? undefined,
          }),
        );
        const done = after ? await after() : false;
        if (done === true && once && created >= 3) {
          console.log(`opened ${created}`);
          socket.close();
        }
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
  if (once) break;
  await new Promise((resolve) => setTimeout(resolve, 2000));
}
