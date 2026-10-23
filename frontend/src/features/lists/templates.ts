import type { ChoiceColor, NewColumn } from './listModel';

export type TemplateId = 'blank' | 'project' | 'bugs' | 'oncall';

export interface ListTemplate {
  id: TemplateId;
  label: string;
  description: string;
  title: string;
  columns: NewColumn[];
}

const choices = (...entries: Array<[string, ChoiceColor]>) => ({
  choices: entries.map(([label, color]) => ({ label, color })),
});

export const LIST_TEMPLATES: ListTemplate[] = [
  {
    id: 'blank',
    label: 'Blank',
    description: 'One text column; add the rest as you go.',
    title: 'Untitled list',
    columns: [{ name: 'Title', kind: 'text' }],
  },
  {
    id: 'project',
    label: 'Project tracker',
    description: 'Tasks with a status, an owner and a due date.',
    title: 'Project tracker',
    columns: [
      { name: 'Task', kind: 'text' },
      {
        name: 'Status',
        kind: 'select',
        options: choices(
          ['Not started', 'gray'],
          ['In progress', 'blue'],
          ['Blocked', 'red'],
          ['Done', 'green'],
        ),
      },
      { name: 'Owner', kind: 'person' },
      { name: 'Due', kind: 'date' },
      {
        name: 'Priority',
        kind: 'select',
        options: choices(['Low', 'gray'], ['Medium', 'yellow'], ['High', 'red']),
      },
    ],
  },
  {
    id: 'bugs',
    label: 'Bug intake',
    description: 'Reports with a severity, an assignee and a link.',
    title: 'Bug intake',
    columns: [
      { name: 'Summary', kind: 'text' },
      {
        name: 'Severity',
        kind: 'select',
        options: choices(['S1', 'red'], ['S2', 'orange'], ['S3', 'yellow'], ['S4', 'gray']),
      },
      {
        name: 'Status',
        kind: 'select',
        options: choices(['New', 'blue'], ['Triaged', 'purple'], ['Fixing', 'yellow'], ['Fixed', 'green']),
      },
      { name: 'Reporter', kind: 'person' },
      { name: 'Assignee', kind: 'person' },
      { name: 'Reported', kind: 'date' },
      { name: 'Link', kind: 'link' },
    ],
  },
  {
    id: 'oncall',
    label: 'On-call rota',
    description: 'Who is primary and secondary, week by week.',
    title: 'On-call rota',
    columns: [
      { name: 'Week of', kind: 'date' },
      { name: 'Primary', kind: 'person' },
      { name: 'Secondary', kind: 'person' },
      { name: 'Confirmed', kind: 'checkbox' },
      { name: 'Notes', kind: 'text' },
    ],
  },
];
