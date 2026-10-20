import { toast } from '@/shared/components/Toast';
import { MAX_UPLOAD_SIZE_BYTES, ErrorLabels } from '@/shared/constants';

export function transferFiles(transfer: DataTransfer | null | undefined): File[] {
  if (!transfer) return [];
  const direct = Array.from(transfer.files ?? []);
  if (direct.length > 0) return direct;
  return Array.from(transfer.items ?? [])
    .filter((item) => item.kind === 'file')
    .map((item) => item.getAsFile())
    .filter((file): file is File => file !== null);
}

export async function uploadFilesSequentially(
  files: File[],
  upload: (file: File) => Promise<void>,
): Promise<void> {
  for (const file of files) {
    if (file.size > MAX_UPLOAD_SIZE_BYTES) {
      toast.error(ErrorLabels.UploadTooLarge);
      continue;
    }
    await upload(file);
  }
}
