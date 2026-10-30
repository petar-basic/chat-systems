import { useState } from 'react';
import type { Workspace } from '@/stores/workspace';
import { workspaceInitials } from '../workspaceTile';

interface WorkspaceIconProps {
  workspace: Pick<Workspace, 'name' | 'icon_url'>;
  className: string;
  initialsClassName: string;
}

export function WorkspaceIcon({ workspace, className, initialsClassName }: WorkspaceIconProps) {
  const [brokenUrl, setBrokenUrl] = useState<string | null>(null);
  const url = workspace.icon_url && workspace.icon_url !== brokenUrl ? workspace.icon_url : null;

  return (
    <span
      className={`${className} overflow-hidden flex items-center justify-center ${url ? 'bg-raised' : initialsClassName}`}
    >
      {url ? (
        <img
          src={url}
          alt=""
          data-qa="workspace-icon-image"
          onError={() => setBrokenUrl(url)}
          className="w-full h-full object-cover"
        />
      ) : (
        workspaceInitials(workspace.name)
      )}
    </span>
  );
}
