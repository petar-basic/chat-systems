import { useCallback, useState } from 'react';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';
import { ErrorLabels } from '@/shared/constants';
import { logger } from '@/lib/logger';
import { toast } from '@/shared/components/Toast';

interface Args {
  workspaceId: string | undefined;
  instanceUrl: string | undefined;
  send: (content: string) => unknown;
}

interface Uploaded {
  filename: string;
  url: string;
}

/// One upload path for every composer: the file goes up, and each stored file
/// is posted as a message the renderer turns back into an attachment card.
export function useAttachmentUpload({ workspaceId, instanceUrl, send }: Args) {
  const [uploading, setUploading] = useState(false);

  const handleFileUpload = useCallback(
    async (file: File) => {
      if (!workspaceId) return;
      setUploading(true);
      try {
        const formData = new FormData();
        formData.append('file', file);
        const uploaded = await getApiForInstance(instanceUrl).upload<Uploaded[]>(
          `/files/upload/${workspaceId}`,
          formData,
        );
        for (const f of uploaded) {
          await send(`[file: ${f.filename}](${f.url})`);
        }
      } catch (err) {
        logger.error('useAttachmentUpload', 'handleFileUpload', err);
        toast.error(ErrorLabels.UploadFailed);
      } finally {
        setUploading(false);
      }
    },
    [workspaceId, instanceUrl, send],
  );

  return { uploading, handleFileUpload };
}
