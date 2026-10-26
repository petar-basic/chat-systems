import { blankDefinition, blankTrigger, type Definition } from './workflowModel';

export type TemplateId = 'blank' | 'welcome' | 'standup' | 'incident' | 'time_off';

export interface Starter {
  name: string;
  description: string;
  definition: Definition;
}

export interface WorkflowTemplate {
  id: TemplateId;
  label: string;
  description: string;
  starter: (channelId: string) => Starter;
}

export const WORKFLOW_TEMPLATES: WorkflowTemplate[] = [
  {
    id: 'blank',
    label: 'Blank',
    description: 'A schedule and one message; change both as you like.',
    starter: (channelId) => ({ name: '', description: '', definition: blankDefinition(channelId) }),
  },
  {
    id: 'welcome',
    label: 'Welcome new members',
    description: 'DM everyone who joins a channel.',
    starter: (channelId) => ({
      name: 'Welcome new members',
      description: 'Greets everyone who joins the channel.',
      definition: {
        trigger: blankTrigger('member_joined', channelId),
        steps: [
          {
            type: 'send_dm',
            to: { kind: 'trigger_user' },
            text: 'Welcome to {{trigger.channel}}, {{trigger.user}}! Have a look at the pinned messages to get started.',
          },
        ],
      },
    }),
  },
  {
    id: 'standup',
    label: 'Daily standup reminder',
    description: 'Post a reminder every weekday before standup.',
    starter: (channelId) => ({
      name: 'Daily standup reminder',
      description: '',
      definition: {
        trigger: { ...blankTrigger('schedule'), time: '09:50' },
        steps: [{ type: 'send_message', channel_id: channelId, text: 'Standup in 10 minutes!' }],
      },
    }),
  },
  {
    id: 'incident',
    label: 'Incident from a reaction',
    description: 'Turn a 🚨 reaction into a post in another channel.',
    starter: (channelId) => ({
      name: 'Incident from a reaction',
      description: 'Reposts a message someone reacts to with 🚨.',
      definition: {
        trigger: { type: 'reaction_added', channel_id: channelId, emoji: '🚨' },
        steps: [
          {
            type: 'send_message',
            channel_id: channelId,
            text: '🚨 {{trigger.user}} flagged a possible incident: {{trigger.message_link}}\n\n{{trigger.message}}',
          },
        ],
      },
    }),
  },
  {
    id: 'time_off',
    label: 'Time-off request into a list',
    description: 'A shortcut asks for the dates and adds them to a list.',
    starter: (channelId) => ({
      name: 'Request time off',
      description: 'Collects a time-off request and tracks it in a list.',
      definition: {
        trigger: blankTrigger('shortcut', channelId),
        steps: [
          {
            type: 'form',
            to: { kind: 'trigger_user' },
            title: 'Request time off',
            fields: [
              {
                name: 'kind',
                label: 'Kind',
                kind: 'select',
                options: ['Holiday', 'Sick', 'Other'],
                required: true,
              },
              { name: 'from', label: 'From', kind: 'date', options: [], required: true },
              { name: 'until', label: 'Until', kind: 'date', options: [], required: true },
              { name: 'note', label: 'Note', kind: 'long_text', options: [], required: false },
            ],
          },
          { type: 'add_list_item', list_id: '', cells: [] },
          {
            type: 'send_message',
            channel_id: channelId,
            text: '{{trigger.user}} asked for time off',
            summary: true,
          },
        ],
      },
    }),
  },
];
