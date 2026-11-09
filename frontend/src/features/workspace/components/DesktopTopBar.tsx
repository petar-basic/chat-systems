import type { CSSProperties } from 'react';
import { useNavigate } from 'react-router';
import { useTranslation } from 'react-i18next';
import { ArrowLeft, ArrowRight, Search } from 'lucide-react';
import { IconButton } from '@/shared/components/IconButton/IconButton';
import { useHistoryReach } from '../hooks/useHistoryReach';

interface Props {
  workspaceName: string | undefined;
  searching: boolean;
  onSearch: () => void;
  style?: CSSProperties;
}

export function DesktopTopBar({ workspaceName, searching, onSearch, style }: Props) {
  const { t } = useTranslation('common');
  const navigate = useNavigate();
  const { back, forward } = useHistoryReach();
  const label = workspaceName ? t('topBar.searchIn', { name: workspaceName }) : t('topBar.search');

  return (
    <header
      data-tauri-drag-region="deep"
      data-qa="desktop-top-bar"
      style={style}
      className="grid h-10 shrink-0 select-none grid-cols-[1fr_minmax(0,40rem)_1fr] items-center gap-3 bg-rail px-2 text-fg"
    >
      <div className="flex items-center gap-0.5">
        <IconButton
          label={t('topBar.back')}
          icon={<ArrowLeft className="h-4 w-4" />}
          disabled={!back}
          onClick={() => void navigate(-1)}
          data-qa="desktop-back"
        />
        <IconButton
          label={t('topBar.forward')}
          icon={<ArrowRight className="h-4 w-4" />}
          disabled={!forward}
          onClick={() => void navigate(1)}
          data-qa="desktop-forward"
        />
      </div>
      <button
        type="button"
        onClick={onSearch}
        aria-pressed={searching}
        data-qa="desktop-search"
        className="flex h-7 min-w-0 cursor-pointer items-center gap-2 rounded-md border border-line bg-raised/40 px-2.5 text-left text-sm text-muted transition hover:bg-raised/70 hover:text-fg-soft focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-purple-500 aria-pressed:bg-raised aria-pressed:text-fg"
      >
        <Search className="h-3.5 w-3.5 shrink-0" />
        <span className="truncate">{label}</span>
      </button>
      <div />
    </header>
  );
}
