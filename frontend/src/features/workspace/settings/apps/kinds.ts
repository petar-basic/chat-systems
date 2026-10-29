import {
  ArrowDownToLine,
  ArrowUpFromLine,
  Bot,
  Plug,
  SquareSlash,
  Workflow,
  type LucideIcon,
} from 'lucide-react';
import type { InstalledKind } from '@/features/workspace/hooks/useAppDirectory';

export const KINDS: Record<InstalledKind, { label: string; Icon: LucideIcon }> = {
  connected_app: { label: 'Connected app', Icon: Plug },
  bot_app: { label: 'App', Icon: Bot },
  incoming_webhook: { label: 'Incoming webhook', Icon: ArrowDownToLine },
  outgoing_webhook: { label: 'Outgoing webhook', Icon: ArrowUpFromLine },
  slash_command: { label: 'Slash command', Icon: SquareSlash },
  workflow: { label: 'Workflow', Icon: Workflow },
};
