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

export const KINDS: Record<InstalledKind, LucideIcon> = {
  connected_app: Plug,
  bot_app: Bot,
  incoming_webhook: ArrowDownToLine,
  outgoing_webhook: ArrowUpFromLine,
  slash_command: SquareSlash,
  workflow: Workflow,
};
