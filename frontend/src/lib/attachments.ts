const FILE_LINK = /^\[file:\s*(.+?)\]\((\/[^)]+|https?:\/\/[^)]+)\)\s*$/;
const IMAGE_EXT = /\.(png|jpe?g|gif|webp|svg|avif|bmp)$/i;

export interface ParsedAttachment {
  filename: string;
  url: string;
  isImage: boolean;
}

export function parseAttachment(content: string): ParsedAttachment | null {
  const match = content.trim().match(FILE_LINK);
  if (!match) return null;
  const [, filename, url] = match;
  const pathname = url.split('?')[0];
  return { filename, url, isImage: IMAGE_EXT.test(filename) || IMAGE_EXT.test(pathname) };
}
