export interface VirtualRangeOptions {
  itemCount: number;
  itemHeight: number;
  scrollTop: number;
  viewportHeight: number;
  overscan?: number;
}

export interface VirtualRange {
  start: number;
  end: number;
}

/** Compute the half-open item range that should be mounted for a fixed-height list. */
export function computeVirtualRange({
  itemCount,
  itemHeight,
  scrollTop,
  viewportHeight,
  overscan = 6,
}: VirtualRangeOptions): VirtualRange {
  const count = Number.isFinite(itemCount) ? Math.max(0, Math.floor(itemCount)) : 0;
  if (count === 0) return { start: 0, end: 0 };

  const height = Number.isFinite(itemHeight) ? Math.max(1, itemHeight) : 1;
  const viewport = Number.isFinite(viewportHeight) ? Math.max(height, viewportHeight) : height;
  const buffer = Number.isFinite(overscan) ? Math.max(0, Math.floor(overscan)) : 0;
  const maxScrollTop = Math.max(0, count * height - viewport);
  const top = Number.isFinite(scrollTop) ? Math.min(maxScrollTop, Math.max(0, scrollTop)) : 0;
  const firstVisible = Math.floor(top / height);
  const visibleEnd = Math.ceil((top + viewport) / height);

  return {
    start: Math.max(0, firstVisible - buffer),
    end: Math.min(count, visibleEnd + buffer),
  };
}
