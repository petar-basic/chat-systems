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

export interface FileKind {
  label: string;
  icon: LucideIcon;
  tone: string;
}

const KINDS: { test: (mime: string, ext: string) => boolean; kind: FileKind }[] = [
  {
    test: (mime, ext) => mime === 'application/pdf' || ext === 'pdf',
    kind: { label: 'PDF', icon: FileText, tone: 'bg-red-500/15 text-red-400' },
  },
  {
    test: (mime) => mime.startsWith('image/'),
    kind: { label: 'Image', icon: FileImage, tone: 'bg-purple-500/15 text-purple-400' },
  },
  {
    test: (mime) => mime.startsWith('video/'),
    kind: { label: 'Video', icon: FileVideo, tone: 'bg-pink-500/15 text-pink-400' },
  },
  {
    test: (mime) => mime.startsWith('audio/'),
    kind: { label: 'Audio', icon: FileAudio, tone: 'bg-amber-500/15 text-amber-400' },
  },
  {
    test: (mime, ext) =>
      mime.includes('spreadsheet') ||
      mime.includes('excel') ||
      ['xls', 'xlsx', 'csv', 'ods', 'numbers'].includes(ext),
    kind: { label: 'Spreadsheet', icon: FileSpreadsheet, tone: 'bg-green-500/15 text-green-400' },
  },
  {
    test: (mime, ext) =>
      mime.includes('presentation') ||
      mime.includes('powerpoint') ||
      ['ppt', 'pptx', 'odp', 'key'].includes(ext),
    kind: { label: 'Presentation', icon: Presentation, tone: 'bg-orange-500/15 text-orange-400' },
  },
  {
    test: (mime, ext) =>
      mime.includes('word') ||
      mime.includes('opendocument.text') ||
      ['doc', 'docx', 'odt', 'rtf', 'pages'].includes(ext),
    kind: { label: 'Document', icon: FileText, tone: 'bg-blue-500/15 text-blue-400' },
  },
  {
    test: (mime, ext) =>
      mime.includes('zip') ||
      mime.includes('compressed') ||
      mime.includes('tar') ||
      ['zip', 'rar', '7z', 'gz', 'tgz', 'tar'].includes(ext),
    kind: { label: 'Archive', icon: FileArchive, tone: 'bg-yellow-500/15 text-yellow-400' },
  },
  {
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
    kind: { label: 'Text', icon: FileCode, tone: 'bg-cyan-500/15 text-cyan-400' },
  },
];

const GENERIC: FileKind = { label: 'File', icon: File, tone: 'bg-raised text-fg-dim' };

export function fileKind(mimeType: string, filename: string): FileKind {
  const ext = filename.includes('.') ? (filename.split('.').pop() ?? '').toLowerCase() : '';
  const mime = mimeType.toLowerCase();
  return KINDS.find(({ test }) => test(mime, ext))?.kind ?? GENERIC;
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
