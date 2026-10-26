import {
  File,
  FileArchive,
  FileAudio,
  FileCode,
  FileImage,
  FileSpreadsheet,
  FileText,
  FileVideo,
  Presentation,
  type LucideIcon,
} from 'lucide-react';
import type { components } from '@/api/schema';

export type FileKindName = components['schemas']['FileKind'];

export interface FileKind {
  label: string;
  icon: LucideIcon;
  tone: string;
}

export const FILE_KINDS: Record<FileKindName, FileKind> = {
  pdf: { label: 'PDF', icon: FileText, tone: 'bg-red-500/15 text-red-400' },
  image: { label: 'Image', icon: FileImage, tone: 'bg-purple-500/15 text-purple-400' },
  video: { label: 'Video', icon: FileVideo, tone: 'bg-pink-500/15 text-pink-400' },
  audio: { label: 'Audio', icon: FileAudio, tone: 'bg-amber-500/15 text-amber-400' },
  spreadsheet: { label: 'Spreadsheet', icon: FileSpreadsheet, tone: 'bg-green-500/15 text-green-400' },
  presentation: { label: 'Presentation', icon: Presentation, tone: 'bg-orange-500/15 text-orange-400' },
  document: { label: 'Document', icon: FileText, tone: 'bg-blue-500/15 text-blue-400' },
  archive: { label: 'Archive', icon: FileArchive, tone: 'bg-yellow-500/15 text-yellow-400' },
  text: { label: 'Text', icon: FileCode, tone: 'bg-cyan-500/15 text-cyan-400' },
  other: { label: 'File', icon: File, tone: 'bg-raised text-fg-dim' },
};

const RULES: { kind: FileKindName; test: (mime: string, ext: string) => boolean }[] = [
  { kind: 'pdf', test: (mime, ext) => mime === 'application/pdf' || ext === 'pdf' },
  { kind: 'image', test: (mime) => mime.startsWith('image/') },
  { kind: 'video', test: (mime) => mime.startsWith('video/') },
  { kind: 'audio', test: (mime) => mime.startsWith('audio/') },
  {
    kind: 'spreadsheet',
    test: (mime, ext) =>
      mime.includes('spreadsheet') ||
      mime.includes('excel') ||
      ['xls', 'xlsx', 'csv', 'ods', 'numbers'].includes(ext),
  },
  {
    kind: 'presentation',
    test: (mime, ext) =>
      mime.includes('presentation') ||
      mime.includes('powerpoint') ||
      ['ppt', 'pptx', 'odp', 'key'].includes(ext),
  },
  {
    kind: 'document',
    test: (mime, ext) =>
      mime.includes('word') ||
      mime.includes('opendocument.text') ||
      ['doc', 'docx', 'odt', 'rtf', 'pages'].includes(ext),
  },
  {
    kind: 'archive',
    test: (mime, ext) =>
      mime.includes('zip') ||
      mime.includes('compressed') ||
      mime.includes('tar') ||
      ['zip', 'rar', '7z', 'gz', 'tgz', 'tar'].includes(ext),
  },
  {
    kind: 'text',
    test: (mime, ext) =>
      mime.startsWith('text/') ||
      mime.includes('json') ||
      mime.includes('xml') ||
      [
        'js',
        'ts',
        'tsx',
        'rs',
        'py',
        'go',
        'java',
        'rb',
        'sh',
        'sql',
        'yml',
        'yaml',
        'toml',
        'md',
        'log',
      ].includes(ext),
  },
];

export function kindOf(mimeType: string, filename: string): FileKindName {
  const ext = filename.includes('.') ? (filename.split('.').pop() ?? '').toLowerCase() : '';
  const mime = mimeType.toLowerCase();
  return RULES.find(({ test }) => test(mime, ext))?.kind ?? 'other';
}

export function fileKind(mimeType: string, filename: string): FileKind {
  return FILE_KINDS[kindOf(mimeType, filename)];
}

const RENDERABLE_IMAGES = new Set([
  'image/png',
  'image/jpeg',
  'image/gif',
  'image/webp',
  'image/avif',
  'image/svg+xml',
]);

export function isRenderableImage(mimeType: string): boolean {
  return RENDERABLE_IMAGES.has(mimeType.toLowerCase());
}

export type MediaKind = 'video' | 'audio';

export function playableMedia(mimeType: string): MediaKind | null {
  const mime = mimeType.toLowerCase();
  const kind: MediaKind | null = mime.startsWith('video/')
    ? 'video'
    : mime.startsWith('audio/')
      ? 'audio'
      : null;
  if (!kind || typeof document === 'undefined') return null;
  return document.createElement(kind).canPlayType(mime) ? kind : null;
}
