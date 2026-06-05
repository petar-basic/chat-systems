import StarterKit from '@tiptap/starter-kit';
import Underline from '@tiptap/extension-underline';
import Link from '@tiptap/extension-link';
import { Markdown } from 'tiptap-markdown';
import { EmojiShortcodes } from './emojiShortcodes';
import { EmojiSuggestion } from '@/features/messaging/emojiSuggestion';

const LINK_PROTOCOLS = ['http', 'https', 'mailto'];
const LINK_ATTRS = { rel: 'noopener noreferrer nofollow', target: '_blank' };

export function createEditorExtensions() {
  return [
    StarterKit.configure({ heading: false, link: false, underline: false }),
    Underline,
    Link.extend({ inclusive: false }).configure({
      openOnClick: false,
      autolink: true,
      linkOnPaste: true,
      protocols: LINK_PROTOCOLS,
      HTMLAttributes: LINK_ATTRS,
    }),
    Markdown.configure({ html: false, tightLists: true, transformPastedText: true }),
    EmojiShortcodes,
    EmojiSuggestion,
  ];
}

export function createDisplayExtensions() {
  return [
    StarterKit.configure({ heading: false, link: false, underline: false }),
    Underline,
    Link.configure({
      openOnClick: true,
      autolink: true,
      protocols: LINK_PROTOCOLS,
      HTMLAttributes: LINK_ATTRS,
    }),
    Markdown.configure({ html: false }),
  ];
}
