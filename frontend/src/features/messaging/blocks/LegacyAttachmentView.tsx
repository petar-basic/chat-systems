import type { components } from '@/api/schema';
import { CachedImage } from '@/shared/components/Image/CachedImage';
import { DAY_AND_TIME, formatClock } from '@/lib/datetime';
import MessageContent from '../MessageContent';
import { BlockView } from './BlockView';
import { safeColour, safeLink } from './layout';

type LegacyAttachment = components['schemas']['LegacyAttachment'];

interface Props {
  attachment: LegacyAttachment;
}

function Linked({ href, className, children }: { href: string | null; className: string; children: string }) {
  if (!href) return <span className={className}>{children}</span>;
  return (
    <a
      href={href}
      target="_blank"
      rel="noopener noreferrer nofollow"
      className={`${className} hover:underline`}
    >
      {children}
    </a>
  );
}

function sentAt(seconds: number) {
  const date = new Date(seconds * 1000);
  const year = date.getFullYear() === new Date().getFullYear() ? undefined : 'numeric';
  return { iso: date.toISOString(), label: formatClock(date, { ...DAY_AND_TIME, year }) };
}

export function LegacyAttachmentView({ attachment }: Props) {
  const colour = safeColour(attachment.color);
  const sent = attachment.ts ? sentAt(attachment.ts) : null;
  const fields = attachment.fields ?? [];
  const blocks = attachment.blocks ?? [];
  return (
    <div className="flex flex-col gap-1" data-qa="legacy-attachment">
      {attachment.pretext && <MessageContent content={attachment.pretext} />}
      <div
        className={`flex gap-3 rounded-sm border-l-4 py-0.5 pl-3 ${colour ? '' : 'border-line-strong'}`}
        style={colour ? { borderLeftColor: colour } : undefined}
        data-qa="legacy-attachment-bar"
        data-colour={colour ?? undefined}
      >
        <div className="flex min-w-0 flex-1 flex-col gap-1">
          {attachment.author_name && (
            <Linked href={safeLink(attachment.author_link)} className="text-xs font-semibold text-fg-dim">
              {attachment.author_name}
            </Linked>
          )}
          {attachment.title && (
            <Linked href={safeLink(attachment.title_link)} className="font-semibold text-accent">
              {attachment.title}
            </Linked>
          )}
          {attachment.text && <MessageContent content={attachment.text} />}
          {fields.length > 0 && (
            <dl className="grid grid-cols-2 gap-x-6 gap-y-1.5" data-qa="legacy-fields">
              {fields.map((field, index) => (
                <div key={index} className={`min-w-0 ${field.short ? '' : 'col-span-2'}`}>
                  {field.title && <dt className="text-xs font-semibold text-fg">{field.title}</dt>}
                  <dd>
                    <MessageContent content={field.value} />
                  </dd>
                </div>
              ))}
            </dl>
          )}
          {blocks.map((block, index) => (
            <BlockView key={index} block={block} />
          ))}
          {attachment.image_url && (
            <CachedImage
              src={attachment.image_url}
              alt=""
              referrerPolicy="no-referrer"
              className="max-h-80 max-w-full self-start rounded-lg border border-line/60 object-contain"
            />
          )}
          {(attachment.footer || sent) && (
            <div className="flex flex-wrap items-center gap-1.5 text-xs text-muted" data-qa="legacy-footer">
              {attachment.footer && <MessageContent content={attachment.footer} />}
              {attachment.footer && sent && <span aria-hidden>·</span>}
              {sent && <time dateTime={sent.iso}>{sent.label}</time>}
            </div>
          )}
        </div>
        {attachment.thumb_url && (
          <CachedImage
            src={attachment.thumb_url}
            alt=""
            referrerPolicy="no-referrer"
            className="h-16 w-16 shrink-0 rounded-md object-cover"
          />
        )}
      </div>
    </div>
  );
}
