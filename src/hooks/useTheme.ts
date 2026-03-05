import { useEffect } from "react";
import { useAppStore } from "@/stores/appStore";

export function useTheme() {
  const themePreference = useAppStore((s) => s.themePreference);
  const setResolvedTheme = useAppStore((s) => s.setResolvedTheme);

  useEffect(() => {
    const mq = window.matchMedia("(prefers-color-scheme: dark)");

    const resolve = () => {
      if (themePreference === "system") {
        setResolvedTheme(mq.matches ? "dark" : "light");
      } else {
        setResolvedTheme(themePreference);
      }
    };

    resolve();
    mq.addEventListener("change", resolve);
    return () => mq.removeEventListener("change", resolve);
  }, [themePreference, setResolvedTheme]);
}
