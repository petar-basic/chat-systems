export function transferFiles(transfer: DataTransfer | null | undefined): File[] {
  if (!transfer) return [];
  const direct = Array.from(transfer.files ?? []);
  if (direct.length > 0) return direct;
  return Array.from(transfer.items ?? [])
    .filter((item) => item.kind === 'file')
    .map((item) => item.getAsFile())
    .filter((file): file is File => file !== null);
}
