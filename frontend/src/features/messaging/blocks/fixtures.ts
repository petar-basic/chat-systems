import type { Message, MessageMetadata } from '@/stores/workspace';

export function laidOut(content: string, metadata: MessageMetadata): Message {
  return {
    id: 'm-1',
    channel_id: 'ch-1',
    user_id: 'u-hook',
    content,
    created_at: '2026-11-17T10:00:00Z',
    updated_at: '2026-11-17T10:00:00Z',
    reply_count: 0,
    is_pinned: false,
    reactions: [],
    metadata,
  } as Message;
}

export const githubActions = laidOut('GitHub Action build result: success', {
  bot: { name: 'GitHub', icon_url: null },
  blocks: [
    { type: 'header', text: { type: 'plain_text', text: '✅ Build passed' } },
    {
      type: 'section',
      fields: [
        { type: 'mrkdwn', text: '**Repository:**\\\n[acme/api](https://github.com/acme/api)' },
        { type: 'mrkdwn', text: '**Branch:**\\\nmain' },
        { type: 'mrkdwn', text: '**Commit:**\\\n[4f2a9c1](https://github.com/acme/api/commit/4f2a9c1)' },
        { type: 'mrkdwn', text: '**Triggered by:**\\\noctocat' },
      ],
    },
    { type: 'divider' },
    {
      type: 'actions',
      elements: [
        {
          type: 'button',
          text: { type: 'plain_text', text: 'View run' },
          url: 'https://github.com/acme/api/actions/runs/9201',
          style: 'primary',
        },
      ],
    },
  ],
});

export const grafana = laidOut(
  '[FIRING:1] High CPU (api-1)\n**Firing**\n\nValue: A=97.2\nLabels:\n - alertname = High CPU\n - instance = api-1\nGrafana v10.4.1',
  {
    bot: { name: 'Grafana', icon_url: null },
    flattened: true,
    legacy_attachments: [
      {
        color: '#d63232',
        title: '[FIRING:1] High CPU (api-1)',
        title_link: 'https://grafana.example.com/alerting/list',
        text: '**Firing**\n\nValue: A=97.2\\\nLabels:\n - alertname = High CPU\n - instance = api-1\n',
        fields: [],
        footer: 'Grafana v10.4.1',
        ts: 1700000000,
        blocks: [],
      },
    ],
  },
);

export const sentry = laidOut("TypeError: Cannot read properties of undefined (reading 'id')", {
  bot: { name: 'Sentry', icon_url: null },
  flattened: true,
  legacy_attachments: [
    {
      color: '#e03e2f',
      title: "TypeError: Cannot read properties of undefined (reading 'id')",
      title_link: 'https://sentry.io/organizations/acme/issues/4512/?referrer=slack',
      text: 'app/services/orders.ts in createOrder',
      fields: [
        { title: 'Project', value: 'backend', short: true },
        { title: 'Environment', value: 'production', short: true },
        { title: 'Level', value: 'error', short: true },
      ],
      footer: '[BACKEND-3QZ](https://sentry.io/organizations/acme/issues/4512/)',
      ts: 1700000123,
      blocks: [],
    },
  ],
});

export const jenkins = laidOut(
  'api-deploy - #42 Success after 3 min 12 sec ([Open](https://ci.example.com/job/api-deploy/42/))',
  {
    bot: { name: 'Jenkins', icon_url: null },
    flattened: true,
    legacy_attachments: [
      {
        color: '#2eb886',
        text: 'api-deploy - #42 Success after 3 min 12 sec ([Open](https://ci.example.com/job/api-deploy/42/))',
        fields: [],
        blocks: [],
      },
    ],
  },
);

export const everyElement = laidOut('Pick a rollout', {
  bot: { name: 'Releases', icon_url: null },
  blocks: [
    {
      type: 'section',
      text: { type: 'mrkdwn', text: 'Release **4.8.0** is ready' },
      accessory: { type: 'image', image_url: 'https://cdn.example.com/logo.png', alt_text: 'Release logo' },
    },
    {
      type: 'actions',
      elements: [
        {
          type: 'button',
          text: { type: 'plain_text', text: 'Approve' },
          action_id: 'approve',
          style: 'primary',
        },
        {
          type: 'button',
          text: { type: 'plain_text', text: 'Reject' },
          action_id: 'reject',
          style: 'danger',
        },
        {
          type: 'static_select',
          placeholder: { type: 'plain_text', text: 'Roll out to' },
          options: [{ text: { type: 'plain_text', text: 'Everyone' }, value: 'all' }],
        },
        { type: 'users_select', placeholder: { type: 'plain_text', text: 'Reviewer' } },
        { type: 'conversations_select' },
        { type: 'datepicker', initial_date: '2026-11-20' },
        {
          type: 'overflow',
          options: [{ text: { type: 'plain_text', text: 'Changelog' }, value: 'log' }],
        },
      ],
    },
    {
      type: 'context',
      elements: [
        { type: 'image', image_url: 'https://cdn.example.com/avatar.png', alt_text: 'Maya' },
        { type: 'mrkdwn', text: 'Asked by **Maya**' },
        { type: 'plain_text', text: '2 minutes ago' },
      ],
    },
    {
      type: 'image',
      image_url: 'https://cdn.example.com/chart.png',
      alt_text: 'Error rate',
      title: { type: 'plain_text', text: 'Error rate, last hour' },
    },
  ],
});
