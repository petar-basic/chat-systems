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

const STEPS = [
  {
    callback_id: "open_ticket",
    title: "Open ticket",
    description: "Files a ticket with the help desk",
    inputs: [
      { name: "title", title: "Title", kind: "text", required: true },
      { name: "reporter", title: "Reporter", kind: "person" },
    ],
    outputs: [
      { name: "ticket_id", title: "Ticket ID", kind: "text", required: true },
    ],
  },
];

let opened = 100;

async function openTicket(event) {
  const id = event.execution_id;
  const title = event.inputs.title ?? "";
  if (/help desk is down/i.test(title)) {
    console.log(`function_executed ${id}: failed`);
    await call(botToken, "POST", `/workflow-executions/${id}/fail`, {
      error: "The help desk is down",
    });
    return;
  }
  opened += 1;
  const ticketId = `HD-${opened}`;
  console.log(`function_executed ${id}: ${ticketId} ${title}`);
  await call(botToken, "POST", `/workflow-executions/${id}/complete`, {
    outputs: { ticket_id: ticketId },
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
      const payload = frame.payload;
      if (frame.type !== "event" || payload.type !== "function_executed")
        return;
      if (payload.event.step.callback_id !== "open_ticket") return;
      try {
        await openTicket(payload.event);
      } catch (error) {
        console.error(error.message);
      }
    });
    socket.addEventListener("close", () => resolve());
    socket.addEventListener("error", () => resolve());
  });
}

const me = await call(botToken, "GET", "/bot-apps/me");
await call(botToken, "PUT", "/bot-apps/me/workflow-steps", { steps: STEPS });
console.log(`${me.name} offers ${STEPS.map((step) => step.title).join(", ")}`);

for (;;) {
  const { url } = await call(appToken, "POST", "/apps/connections.open");
  await connect(url);
  await new Promise((resolve) => setTimeout(resolve, 2000));
}
