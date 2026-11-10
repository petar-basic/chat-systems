import type { Editor } from '@tiptap/react';
import { useTranslation } from 'react-i18next';
import { useEditorState } from '@tiptap/react';
import { Bold, Italic, Underline, Strikethrough, Code, Code2, List, ListOrdered, Quote } from 'lucide-react';
import { isApplePlatform, shortcutLabel } from '@/features/navigation';

interface Props {
  editor: Editor | null;
}

interface ToolbarButtonProps {
  onClick: () => void;
  active?: boolean;
  label: string;
  children: React.ReactNode;
}

function ToolbarButton({ onClick, active, label, children }: ToolbarButtonProps) {
  return (
    <button
      type="button"
      onMouseDown={(e) => {
        e.preventDefault();
        onClick();
      }}
      aria-label={label}
      data-tooltip={label}
      className={`p-1 rounded transition cursor-pointer ${
        active ? 'text-accent bg-purple-500/15' : 'text-muted hover:text-fg-soft hover:bg-raised/60'
      }`}
    >
      {children}
    </button>
  );
}

function Divider() {
  return <div className="w-px h-4 bg-elevated/60 mx-0.5" />;
}

export default function FormattingToolbar({ editor }: Props) {
  const { t } = useTranslation('messages');
  const apple = isApplePlatform();
  const activeMarks = useEditorState({
    editor,
    selector: (ctx) => ({
      bold: ctx.editor?.isActive('bold') ?? false,
      italic: ctx.editor?.isActive('italic') ?? false,
      underline: ctx.editor?.isActive('underline') ?? false,
      strike: ctx.editor?.isActive('strike') ?? false,
    }),
  });

  if (!editor) return null;

  return (
    <div className="flex items-center gap-0.5">
      <ToolbarButton
        onClick={() => editor.chain().focus().toggleBold().run()}
        active={activeMarks?.bold}
        label={t('toolbar.bold', { keys: shortcutLabel(['mod', 'B'], apple) })}
      >
        <Bold className="w-3.5 h-3.5" />
      </ToolbarButton>
      <ToolbarButton
        onClick={() => editor.chain().focus().toggleItalic().run()}
        active={activeMarks?.italic}
        label={t('toolbar.italic', { keys: shortcutLabel(['mod', 'I'], apple) })}
      >
        <Italic className="w-3.5 h-3.5" />
      </ToolbarButton>
      <ToolbarButton
        onClick={() => editor.chain().focus().toggleUnderline().run()}
        active={activeMarks?.underline}
        label={t('toolbar.underline', { keys: shortcutLabel(['mod', 'U'], apple) })}
      >
        <Underline className="w-3.5 h-3.5" />
      </ToolbarButton>
      <ToolbarButton
        onClick={() => editor.chain().focus().toggleStrike().run()}
        active={activeMarks?.strike}
        label={t('toolbar.strike')}
      >
        <Strikethrough className="w-3.5 h-3.5" />
      </ToolbarButton>
      <Divider />
      <ToolbarButton onClick={() => editor.chain().focus().toggleCode().run()} label={t('toolbar.code')}>
        <Code className="w-3.5 h-3.5" />
      </ToolbarButton>
      <ToolbarButton
        onClick={() => editor.chain().focus().toggleCodeBlock().run()}
        label={t('toolbar.codeBlock')}
      >
        <Code2 className="w-3.5 h-3.5" />
      </ToolbarButton>
      <Divider />
      <ToolbarButton
        onClick={() => editor.chain().focus().toggleBulletList().run()}
        label={t('toolbar.bullets')}
      >
        <List className="w-3.5 h-3.5" />
      </ToolbarButton>
      <ToolbarButton
        onClick={() => editor.chain().focus().toggleOrderedList().run()}
        label={t('toolbar.numbers')}
      >
        <ListOrdered className="w-3.5 h-3.5" />
      </ToolbarButton>
      <ToolbarButton
        onClick={() => editor.chain().focus().toggleBlockquote().run()}
        label={t('toolbar.quote')}
      >
        <Quote className="w-3.5 h-3.5" />
      </ToolbarButton>
    </div>
  );
}
