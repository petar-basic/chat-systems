import { useWorkspaceStore } from '@/stores/workspace';
import { instanceManager } from '@/lib/instances';
import { api, type ApiClient } from '@/lib/api';

export function getApiForInstance(instanceUrl?: string | null): ApiClient {
  return instanceUrl ? instanceManager.get(instanceUrl).api : api;
}

export function useCurrentApi(): ApiClient {
  const instanceUrl = useWorkspaceStore((s) => s.currentWorkspace?.instanceUrl);
  return getApiForInstance(instanceUrl);
}
