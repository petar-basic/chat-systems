import { Extension } from '@tiptap/core';
import { Plugin, PluginKey } from '@tiptap/pm/state';
import { Decoration, DecorationSet } from '@tiptap/pm/view';
import type { Node } from '@tiptap/pm/model';

export interface MentionRef {
  label: string;
  id: string;
}

const BROADCAST_MENTIONS = ['here', 'everyone', 'channel'];

interface MentionHighlightMeta {
  selfId?: string;
  mentions?: MentionRef[];
}

interface MentionHighlightPluginState {
  decorations: DecorationSet;
  selfId?: string;
  mentions: MentionRef[];
}

function classFor(isSelf: boolean) {
  return isSelf ? 'mention mention-self' : 'mention mention-other';
}

function isStartBoundary(ch: string) {
  return ch === '' || ch === ' ' || ch === '\n' || ch === '(';
}

function isWordChar(ch: string) {
  return /[A-Za-z0-9_]/.test(ch);
}

function buildDecorations(doc: Node, selfId: string | undefined, mentions: MentionRef[]): DecorationSet {
  const decorations: Decoration[] = [];
  const named = [...mentions].sort((a, b) => b.label.length - a.label.length);

  doc.descendants((node, pos) => {
    if (!node.isText) return;
    const text = node.text ?? '';
    if (!text.includes('@')) return;

    const taken: Array<[number, number]> = [];
    const overlaps = (start: number, end: number) => taken.some(([ts, te]) => start < te && end > ts);
    const decorate = (start: number, len: number, isSelf: boolean) => {
      const end = start + len;
      if (overlaps(start, end)) return;
      taken.push([start, end]);
      decorations.push(Decoration.inline(pos + start, pos + end, { class: classFor(isSelf) }));
    };

    for (const mention of named) {
      const needle = `@${mention.label}`;
      let idx = text.indexOf(needle);
      while (idx !== -1) {
        if (isStartBoundary(idx === 0 ? '' : text[idx - 1])) {
          decorate(idx, needle.length, selfId !== undefined && mention.id === selfId);
        }
        idx = text.indexOf(needle, idx + needle.length);
      }
    }

    for (const word of BROADCAST_MENTIONS) {
      const needle = `@${word}`;
      let idx = text.indexOf(needle);
      while (idx !== -1) {
        const before = idx === 0 ? '' : text[idx - 1];
        const afterIdx = idx + needle.length;
        const after = afterIdx < text.length ? text[afterIdx] : '';
        if (isStartBoundary(before) && !isWordChar(after)) {
          decorate(idx, needle.length, true);
        }
        idx = text.indexOf(needle, idx + needle.length);
      }
    }
  });

  return DecorationSet.create(doc, decorations);
}

export const mentionHighlightPluginKey = new PluginKey<MentionHighlightPluginState>('mentionHighlight');

export function createMentionHighlightExtension() {
  return Extension.create({
    name: 'mentionHighlight',
    addProseMirrorPlugins() {
      return [
        new Plugin<MentionHighlightPluginState>({
          key: mentionHighlightPluginKey,
          state: {
            init(_, { doc }): MentionHighlightPluginState {
              return {
                decorations: buildDecorations(doc, undefined, []),
                selfId: undefined,
                mentions: [],
              };
            },
            apply(tr, value): MentionHighlightPluginState {
              const meta = tr.getMeta(mentionHighlightPluginKey) as MentionHighlightMeta | undefined;
              const hasMeta = meta !== undefined;
              const nextSelfId = hasMeta && 'selfId' in meta ? meta.selfId : value.selfId;
              const nextMentions = hasMeta && meta.mentions !== undefined ? meta.mentions : value.mentions;

              if (!tr.docChanged && !hasMeta) return value;

              return {
                decorations: buildDecorations(tr.doc, nextSelfId, nextMentions),
                selfId: nextSelfId,
                mentions: nextMentions,
              };
            },
          },
          props: {
            decorations(state) {
              return mentionHighlightPluginKey.getState(state)?.decorations;
            },
          },
        }),
      ];
    },
  });
}
