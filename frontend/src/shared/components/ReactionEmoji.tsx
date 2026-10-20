import { useCustomEmojiStore } from '@/stores/customEmoji';
import { splitCustomEmoji } from '@/lib/customEmoji';

// A reaction is usually one unicode character, but an imported one can be a
// custom shortcode — Slack lets people react with those, and reading `:shipit:`
// as text is not what anybody meant by it.
export function ReactionEmoji({ emoji }: { emoji: string }) {
  const byName = useCustomEmojiStore((s) => s.byName);
  const spans = splitCustomEmoji(emoji, byName);

  if (spans.length === 1 && spans[0].emoji === null) return <span>{emoji}</span>;

  return (
    <span>
      {spans.map((span, i) =>
        span.emoji === null ? (
          <span key={i}>{span.text}</span>
        ) : (
          <img
            key={i}
            src={span.emoji.url}
            alt={span.text}
            title={span.text}
            data-qa="custom-emoji"
            className="inline-block w-4 h-4 align-text-bottom"
          />
        ),
      )}
    </span>
  );
}
