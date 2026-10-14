import { useEffect, useState } from "react";

/** 订阅 matchMedia 查询，支持 SSR 安全的初始值 */
export function useMediaQuery(query: string): boolean {
  const [matches, setMatches] = useState(
    () => typeof window !== "undefined" && window.matchMedia(query).matches
  );

  useEffect(() => {
    const mql = window.matchMedia(query);
    const onChange = () => setMatches(mql.matches);
    onChange();
    mql.addEventListener("change", onChange);
    return () => mql.removeEventListener("change", onChange);
  }, [query]);

  return matches;
}

/** 移动端断点（与 App.tsx / index.html 的 768px 保持一致） */
export const useIsMobile = () => useMediaQuery("(max-width: 768px)");
