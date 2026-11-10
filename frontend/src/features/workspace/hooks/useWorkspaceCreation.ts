import { useQueries } from '@tanstack/react-query';
import { instanceInfoQuery } from '@/features/instance';
import { instanceManager } from '@/lib/instances';
import { useInstanceStore, type InstanceRole } from '@/stores/instances';

interface CreationRule {
  workspace_creation_restricted?: boolean;
}

export function mayCreateWorkspaces(info: CreationRule | undefined, role: InstanceRole | undefined): boolean {
  if (!info || !role) return false;
  return !info.workspace_creation_restricted || role !== 'member';
}

export function canCreateOn(creation: Record<string, boolean>, instanceUrl: string | undefined): boolean {
  if (!instanceUrl) return false;
  const target = instanceManager.normalize(instanceUrl);
  return Object.entries(creation).some(
    ([url, allowed]) => allowed && instanceManager.normalize(url) === target,
  );
}

export function useWorkspaceCreation(): Record<string, boolean> {
  const instances = useInstanceStore((s) => s.instances);
  return useQueries({
    queries: instances.map((instance) => instanceInfoQuery(instance.url)),
    combine: (results) =>
      Object.fromEntries(
        instances.map((instance, index) => [
          instance.url,
          mayCreateWorkspaces(results[index]?.data, instance.user.instance_role),
        ]),
      ),
  });
}
