import { useState } from 'react';
import { useMediaUrl } from '@/shared/hooks/useMediaUrl';
import type { Workspace } from '@/stores/workspace';
import { workspaceInitials } from '../workspaceTile';

interface WorkspaceIconProps {
  workspace: Pick<Workspace, 'name' | 'icon_url'>;
  className: string;
  initialsClassName: string;
}

export function WorkspaceIcon({ workspace, className, initialsClassName }: WorkspaceIconProps) {
  const [brokenUrl, setBrokenUrl] = useState<string | null>(null);
  const linked = useMediaUrl(workspace.icon_url);
  const src = linked && linked !== brokenUrl ? linked : null;

  return (
    <span
      className={`${className} overflow-hidden flex items-center justify-center ${src ? 'bg-raised' : initialsClassName}`}
    >
      {src ? (
        <img
          src={src}
          alt=""
          data-qa="workspace-icon-image"
          onError={() => setBrokenUrl(src)}
          className="w-full h-full object-cover"
        />
      ) : (
        workspaceInitials(workspace.name)
      )}
    </span>
  );
}
