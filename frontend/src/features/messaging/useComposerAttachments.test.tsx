import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, act, waitFor } from '@testing-library/react';
import { useComposerAttachments, asAttachments } from './useComposerAttachments';

const uploadWithProgress = vi.fn();
const remove = vi.fn();
vi.mock('@/shared/hooks/useCurrentApi', () => ({
  getApiForInstance: () => ({
    uploadWithProgress,
    typed: (op: (client: unknown) => Promise<unknown>) =>
      op({ DELETE: (path: string, init: unknown) => Promise.resolve(remove(path, init)) }),
  }),
}));
vi.mock('@/features/instance/hooks/useInstanceInfo', () => ({
  useInstanceInfo: () => ({ data: { max_upload_bytes: 10 } }),
}));
const toastError = vi.fn();
vi.mock('@/shared/components/Toast', () => ({ toast: { error: (msg: string) => toastError(msg) } }));

const file = (name: string, size = 4, type = 'text/plain') => new File(['x'.repeat(size)], name, { type });

describe('useComposerAttachments', () => {
  beforeEach(() => {
    uploadWithProgress.mockReset();
    remove.mockReset();
    toastError.mockReset();
  });

  it('uploads as a draft, reports progress, and hands ready files to the send', async () => {
    let finish: (value: unknown) => void = () => {};
    uploadWithProgress.mockImplementation(
      (_path: string, _form: FormData, onProgress: (p: number) => void) => {
        onProgress(0.5);
        return new Promise((resolve) => {
          finish = resolve;
        });
      },
    );
    const { result } = renderHook(() =>
      useComposerAttachments({ workspaceId: 'ws-1', instanceUrl: undefined, resetKey: 'ch-1' }),
    );

    act(() => result.current.add([file('notes.txt')]));
    expect(uploadWithProgress.mock.calls[0][0]).toBe('/files/upload/ws-1?draft=true');
    await waitFor(() => expect(result.current.items[0].progress).toBe(0.5));
    expect(result.current.busy).toBe(true);

    await act(async () => {
      finish([
        {
          id: 'file-1',
          url: 'https://chat.test/f',
          filename: 'notes.txt',
          mime_type: 'text/plain',
          size_bytes: 4,
        },
      ]);
    });
    expect(result.current.busy).toBe(false);
    expect(result.current.hasReady).toBe(true);

    let taken: ReturnType<typeof result.current.take> = [];
    act(() => {
      taken = result.current.take();
    });
    expect(asAttachments(taken).map((a) => a.id)).toEqual(['file-1']);
    expect(result.current.items).toHaveLength(0);
  });

  it('refuses files over the server limit before uploading them', () => {
    const { result } = renderHook(() =>
      useComposerAttachments({ workspaceId: 'ws-1', instanceUrl: undefined, resetKey: 'ch-1' }),
    );
    act(() => result.current.add([file('huge.bin', 11)]));
    expect(uploadWithProgress).not.toHaveBeenCalled();
    expect(toastError).toHaveBeenCalledWith('huge.bin is larger than 10 B');
  });

  it('deletes an uploaded draft that is removed from the tray', async () => {
    uploadWithProgress.mockResolvedValue([
      { id: 'file-2', url: 'https://chat.test/g', filename: 'a.txt', mime_type: 'text/plain', size_bytes: 4 },
    ]);
    const { result } = renderHook(() =>
      useComposerAttachments({ workspaceId: 'ws-1', instanceUrl: undefined, resetKey: 'ch-1' }),
    );
    act(() => result.current.add([file('a.txt')]));
    await waitFor(() => expect(result.current.hasReady).toBe(true));
    act(() => result.current.remove(result.current.items[0].localId));
    expect(result.current.items).toHaveLength(0);
    expect(remove).toHaveBeenCalledWith('/files/{file_id}', { params: { path: { file_id: 'file-2' } } });
  });

  it('keeps the files of a channel while another channel is open', async () => {
    uploadWithProgress.mockResolvedValue([
      { id: 'file-3', url: 'https://chat.test/h', filename: 'b.txt', mime_type: 'text/plain', size_bytes: 4 },
    ]);
    const { result, rerender } = renderHook(
      ({ channel }) =>
        useComposerAttachments({ workspaceId: 'ws-1', instanceUrl: undefined, resetKey: channel }),
      { initialProps: { channel: 'ch-keep' } },
    );
    act(() => result.current.add([file('b.txt')]));
    await waitFor(() => expect(result.current.hasReady).toBe(true));

    rerender({ channel: 'ch-other' });
    expect(result.current.items).toHaveLength(0);
    rerender({ channel: 'ch-keep' });
    expect(result.current.items.map((item) => item.fileId)).toEqual(['file-3']);
  });

  it('holds a failed upload with its reason until it is retried', async () => {
    uploadWithProgress.mockRejectedValueOnce(new Error('Storage is full'));
    const { result } = renderHook(() =>
      useComposerAttachments({ workspaceId: 'ws-1', instanceUrl: undefined, resetKey: 'ch-failed' }),
    );
    act(() => result.current.add([file('c.txt')]));
    await waitFor(() => expect(result.current.failed).toBe(true));
    expect(result.current.items[0].error).toBe('Storage is full');

    uploadWithProgress.mockResolvedValueOnce([
      { id: 'file-4', url: 'https://chat.test/i', filename: 'c.txt', mime_type: 'text/plain', size_bytes: 4 },
    ]);
    act(() => result.current.retry(result.current.items[0].localId));
    await waitFor(() => expect(result.current.hasReady).toBe(true));
    expect(result.current.failed).toBe(false);
    expect(uploadWithProgress).toHaveBeenCalledTimes(2);
  });
});
