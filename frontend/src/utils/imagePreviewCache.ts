/**
 * 插图预览 objectURL 会话级缓存（LRU）。
 * 同一生成任务的插图 blob 只下载一次，切换分页/角色时直接复用，避免重复请求。
 */
const MAX_ENTRIES = 24;
const cache = new Map<string, string>();

export function getCachedImagePreview(jobId: string): string | undefined {
  const url = cache.get(jobId);
  if (url) {
    // LRU：命中后移到末尾
    cache.delete(jobId);
    cache.set(jobId, url);
  }
  return url;
}

export function cacheImagePreview(jobId: string, objectUrl: string) {
  if (cache.has(jobId)) cache.delete(jobId);
  cache.set(jobId, objectUrl);
  while (cache.size > MAX_ENTRIES) {
    const oldestKey = cache.keys().next().value;
    if (oldestKey === undefined) break;
    const oldestUrl = cache.get(oldestKey);
    cache.delete(oldestKey);
    if (oldestUrl) window.URL.revokeObjectURL(oldestUrl);
  }
}
