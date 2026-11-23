const api = (process.env.CHAT_API_URL ?? "http://localhost:3000/api/v1").replace(
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
const notesByStack = new Map();

const requestView = (channelId) => ({
  type: "modal",
  callback_id: "deploy-request",
  private_metadata: channelId,
  title: text("Request a deploy"),
  submit: text("Request"),
  close: text("Cancel"),
  blocks: [
    {
      type: "input",
      block_id: "service",
      label: text("Service"),
      hint: text("Its short name, such as api or web"),
      element: {
        type: "plain_text_input",
        action_id: "name",
        placeholder: text("api"),
      },
    },
    {
      type: "input",
      block_id: "target",
      label: text("Environment"),
      element: {
        type: "radio_buttons",
        action_id: "env",
        options: [
          { text: text("Staging"), value: "staging" },
          { text: text("Production"), value: "production" },
        ],
        initial_option: { text: text("Staging"), value: "staging" },
      },
    },
    {
      type: "actions",
      block_id: "more",
      elements: [
        { type: "button", action_id: "notes", text: text("Add release notes") },
      ],
    },
  ],
});

const notesView = {
  type: "modal",
  callback_id: "deploy-notes",
  title: text("Release notes"),
  submit: text("Keep"),
  close: text("Back"),
  blocks: [
    {
      type: "input",
      block_id: "notes",
      label: text("What changes"),
      element: { type: "plain_text_input", action_id: "text", multiline: true },
    },
  ],
};

function decisionBlocks(who, service, env, notes) {
  return [
    {
      type: "section",
      block_id: "ask",
      text: {
        type: "mrkdwn",
        text: `*${who}* asks to deploy *${service}* to *${env}*.`,
      },
    },
    ...(notes
      ? [{ type: "context", elements: [{ type: "plain_text", text: notes }] }]
      : []),
    {
      type: "actions",
      block_id: "decision",
      elements: [
        {
          type: "button",
          action_id: "approve",
          style: "primary",
          value: `${service}@${env}`,
          text: text("Approve"),
        },
        {
          type: "button",
          action_id: "reject",
          style: "danger",
          value: `${service}@${env}`,
          text: text("Reject"),
        },
      ],
    },
  ];
}

async function openRequest(command) {
  await call(botToken, "POST", "/views/open", {
    trigger_id: command.trigger_id,
    view: requestView(command.channel_id),
  });
}

async function pushNotes(payload) {
  await call(botToken, "POST", "/views/push", {
    trigger_id: payload.trigger_id,
    view: notesView,
  });
}

function submitted(payload) {
  const view = payload.view;
  const values = view.state.values;
  if (view.callback_id === "deploy-notes") {
    notesByStack.set(view.root_view_id, values.notes.text.value ?? "");
    return { answer: null };
  }
  const service = (values.service.name.value ?? "").trim();
  if (!/^[a-z0-9-]+$/.test(service)) {
    return {
      answer: {
        response_action: "errors",
        errors: {
          service: "Use the short name: lower-case letters, digits and dashes",
        },
      },
    };
  }
  const env = values.target.env.selected_option?.value ?? "staging";
  const notes = notesByStack.get(view.root_view_id);
  notesByStack.delete(view.root_view_id);
  return {
    answer: { response_action: "clear" },
    after: () =>
      call(botToken, "POST", `/channels/${view.private_metadata}/messages`, {
        content: `${payload.user.name} asks to deploy ${service} to ${env}`,
        blocks: decisionBlocks(
          payload.user.name ?? "Somebody",
          service,
          env,
          notes,
        ),
        client_message_id: crypto.randomUUID(),
      }),
  };
}

async function decide(payload) {
  const action = payload.actions[0];
  const [service, env] = (action.value ?? "").split("@");
  const verdict = action.action_id === "approve" ? "Approved" : "Rejected";
  await fetch(payload.response_url, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      replace_original: true,
      text: `${verdict} by ${payload.user.name}`,
      blocks: [
        {
          type: "section",
          text: { type: "mrkdwn", text: `*${service}* to *${env}*` },
        },
        {
          type: "context",
          elements: [
            { type: "mrkdwn", text: `${verdict} by ${payload.user.name}` },
          ],
        },
      ],
    }),
  });
  return true;
}

function interact(payload) {
  if (payload.type === "view_submission") return submitted(payload);
  if (payload.type !== "block_actions") return { answer: null };
  const action = payload.actions[0];
  if (payload.container.type === "view" && action.action_id === "notes") {
    return { answer: null, after: () => pushNotes(payload) };
  }
  if (payload.container.type === "message" && action.block_id === "decision") {
    return { answer: null, after: () => decide(payload) };
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
        if (frame.type === "slash_command") {
          socket.send(JSON.stringify({ envelope_id: frame.envelope_id }));
          await openRequest(frame.payload);
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
        if (done === true && once) {
          console.log("decided 1");
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
