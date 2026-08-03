import { useEffect, useState } from "react";

/** 输入防抖：value 稳定 delayMs 后才更新返回值，用于搜索框等逐键触发场景。 */
export function useDebouncedValue<T>(value: T, delayMs = 300): T {
  const [debounced, setDebounced] = useState(value);
  useEffect(() => {
    const timer = window.setTimeout(() => setDebounced(value), delayMs);
    return () => window.clearTimeout(timer);
  }, [value, delayMs]);
  return debounced;
}
