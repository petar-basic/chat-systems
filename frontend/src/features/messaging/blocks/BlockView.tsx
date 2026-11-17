import type { components } from '@/api/schema';
import { CachedImage } from '@/shared/components/Image/CachedImage';
import MessageContent from '../MessageContent';
import { BlockElementView } from './BlockElementView';
import { BlockTextView } from './BlockTextView';
import { useBlockActions } from './blockActions';

type Block = components['schemas']['Block'];

interface Props {
  block: Block;
}

export function BlockView({ block }: Props) {
  const actions = useBlockActions();
  const failure =
    actions?.failure && block.block_id && actions.failure.blockId === block.block_id ? (
      <p role="alert" className="text-xs text-danger" data-qa="block-failure">
        {actions.failure.text}
      </p>
    ) : null;

  switch (block.type) {
    case 'header':
      return (
        <h3 className="text-base font-bold text-fg" data-qa="block-header">
          {block.text.text}
        </h3>
      );
    case 'divider':
      return <hr className="border-line" data-qa="block-divider" />;
    case 'image':
      return (
        <figure className="flex flex-col gap-1" data-qa="block-image">
          {block.title && <figcaption className="text-xs text-muted">{block.title.text}</figcaption>}
          <CachedImage
            src={block.image_url}
            alt={block.alt_text}
            referrerPolicy="no-referrer"
            className="max-h-80 max-w-full self-start rounded-lg border border-line/60 object-contain"
          />
        </figure>
      );
    case 'context':
      return (
        <div
          className="flex flex-wrap items-center gap-x-1.5 gap-y-1 text-xs text-muted"
          data-qa="block-context"
        >
          {block.elements.map((element, index) =>
            element.type === 'image' ? (
              <CachedImage
                key={index}
                src={element.image_url}
                alt={element.alt_text}
                referrerPolicy="no-referrer"
                className="h-4 w-4 rounded-sm object-cover"
              />
            ) : element.type === 'mrkdwn' ? (
              <MessageContent key={index} content={element.text} />
            ) : (
              <span key={index} className="whitespace-pre-wrap">
                {element.text}
              </span>
            ),
          )}
        </div>
      );
    case 'actions':
      return (
        <div className="flex flex-col gap-1">
          <div className="flex flex-wrap items-center gap-2" data-qa="block-actions">
            {block.elements.map((element, index) => (
              <BlockElementView key={index} element={element} blockId={block.block_id} />
            ))}
          </div>
          {failure}
        </div>
      );
    case 'input':
      return null;
    case 'section':
      return (
        <div className="flex items-start gap-3" data-qa="block-section">
          <div className="flex min-w-0 flex-1 flex-col gap-2">
            {block.text && <BlockTextView text={block.text} />}
            {!!block.fields?.length && (
              <div className="grid grid-cols-1 gap-x-6 gap-y-2 sm:grid-cols-2" data-qa="block-fields">
                {block.fields.map((field, index) => (
                  <BlockTextView key={index} text={field} className="min-w-0" />
                ))}
              </div>
            )}
            {failure}
          </div>
          {block.accessory && <BlockElementView element={block.accessory} blockId={block.block_id} />}
        </div>
      );
  }
}
