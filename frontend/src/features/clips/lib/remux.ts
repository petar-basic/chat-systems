import type { ClipContainer } from './format';

export interface Seekable {
  blob: Blob;
  durationMs: number;
}

export async function makeSeekable(recording: Blob, container: ClipContainer): Promise<Seekable> {
  const {
    ALL_FORMATS,
    BlobSource,
    BufferTarget,
    Conversion,
    Input,
    Mp4OutputFormat,
    Output,
    WebMOutputFormat,
  } = await import('mediabunny');
  const input = new Input({ source: new BlobSource(recording), formats: ALL_FORMATS });
  try {
    const duration = await input.computeDuration();
    const output = new Output({
      format: container === 'mp4' ? new Mp4OutputFormat({ fastStart: 'in-memory' }) : new WebMOutputFormat(),
      target: new BufferTarget(),
    });
    const conversion = await Conversion.init({ input, output });
    if (!conversion.isValid || conversion.discardedTracks.length > 0) {
      throw new Error(
        `the recording cannot be copied: ${conversion.discardedTracks.map((t) => t.reason).join(', ')}`,
      );
    }
    await conversion.execute();
    const buffer = output.target.buffer;
    if (!buffer) throw new Error('the copy came out empty');
    return { blob: new Blob([buffer], { type: recording.type }), durationMs: Math.round(duration * 1000) };
  } finally {
    input.dispose();
  }
}
