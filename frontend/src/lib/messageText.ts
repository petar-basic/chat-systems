import { parseMessage, type MessageNode } from './messageMarkdown';
import { flattenMentions } from './mentions';

function inlineText(nodes: MessageNode[]): string {
  return nodes
    .map((node) => {
      if (node.kind === 'text' || node.kind === 'code') return node.text;
      if (node.kind === 'break') return '\n';
      return 'children' in node ? inlineText(node.children) : '';
    })
    .join('');
}

function listItemText(marker: string, item: MessageNode): string {
  const body = 'children' in item ? blocksText(item.children) : '';
  return marker + body.replace(/\n/g, `\n${' '.repeat(marker.length)}`);
}

function blockText(node: MessageNode): string {
  switch (node.kind) {
    case 'codeBlock':
      return node.text;
    case 'rule':
      return '';
    case 'blockquote':
    case 'listItem':
      return blocksText(node.children);
    case 'bulletList':
      return node.children.map((item) => listItemText('• ', item)).join('\n');
    case 'orderedList':
      return node.children.map((item, i) => listItemText(`${node.start + i}. `, item)).join('\n');
    default:
      return inlineText([node]);
  }
}

function blocksText(nodes: MessageNode[]): string {
  return nodes.map(blockText).filter(Boolean).join('\n');
}

export function messageText(content: string): string {
  return blocksText(parseMessage(flattenMentions(content))).trim();
}
