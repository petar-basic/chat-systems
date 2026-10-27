import { useState } from 'react';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';

export function usePhotoUpload(instanceUrl?: string, workspaceId?: string) {
  const [uploading, setUploading] = useState(false);

  const upload = async (file: File): Promise<string | null> => {
    if (!workspaceId) return null;
    setUploading(true);
    try {
      const formData = new FormData();
      formData.append('file', file);
      const uploaded = await getApiForInstance(instanceUrl).upload<{ url: string }[]>(
        `/files/upload/${workspaceId}`,
        formData,
      );
      return uploaded[0]?.url ?? null;
    } catch {
      return null;
    } finally {
      setUploading(false);
    }
  };

  return { upload, uploading };
}
