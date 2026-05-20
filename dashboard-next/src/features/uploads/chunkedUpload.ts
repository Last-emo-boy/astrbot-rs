export interface UploadChunk {
  index: number;
  total: number;
  blob: Blob;
  start: number;
  end: number;
}

export interface ChunkPreview {
  index: number;
  start: number;
  end: number;
  size: number;
}

export interface ChunkedUploadProgress {
  uploadedChunks: number;
  totalChunks: number;
  uploadedBytes: number;
  totalBytes: number;
}

export interface ChunkedUploadOptions {
  chunkSize?: number;
  onProgress?: (progress: ChunkedUploadProgress) => void;
}

export const DEFAULT_CHUNK_SIZE = 1024 * 1024;

export function createChunkPreviews(file: File, chunkSize = DEFAULT_CHUNK_SIZE): ChunkPreview[] {
  const total = Math.max(1, Math.ceil(file.size / chunkSize));
  return Array.from({ length: total }, (_, index) => {
    const start = index * chunkSize;
    const end = Math.min(file.size, start + chunkSize);
    return {
      index,
      start,
      end,
      size: end - start,
    };
  });
}

export async function uploadFileInChunks(
  file: File,
  uploadChunk: (chunk: UploadChunk) => Promise<void>,
  options: ChunkedUploadOptions = {}
): Promise<void> {
  const chunkSize = options.chunkSize ?? DEFAULT_CHUNK_SIZE;
  const previews = createChunkPreviews(file, chunkSize);
  let uploadedBytes = 0;

  for (const preview of previews) {
    await uploadChunk({
      index: preview.index,
      total: previews.length,
      blob: file.slice(preview.start, preview.end),
      start: preview.start,
      end: preview.end,
    });
    uploadedBytes += preview.size;
    options.onProgress?.({
      uploadedChunks: preview.index + 1,
      totalChunks: previews.length,
      uploadedBytes,
      totalBytes: file.size,
    });
  }
}
