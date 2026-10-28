import { useState } from 'react';
import type { components } from '@/api/schema';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';

type WorkspaceProfile = components['schemas']['WorkspaceProfile'];

export function usePhotoUpload(instanceUrl?: string, workspaceId?: string) {
  const [uploading, setUploading] = useState(false);

  const upload = async (file: File): Promise<string | null> => {
    if (!workspaceId) return null;
    setUploading(true);
    try {
      const formData = new FormData();
      formData.append('file', file);
      const profile = await getApiForInstance(instanceUrl).upload<WorkspaceProfile>(
        `/workspaces/${workspaceId}/profile/avatar`,
        formData,
      );
      return profile.avatar_url ?? null;
    } catch {
      return null;
    } finally {
      setUploading(false);
    }
  };

  return { upload, uploading };
}
