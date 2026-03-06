import { useEffect, useState, useCallback, useRef } from "react";
import { checkForUpdate } from "@/lib/tauri";
import type { UpdateInfo } from "@/types";

const DISMISSED_VERSION_KEY = "ossue_dismissed_update_version";
const INITIAL_DELAY_MS = 30_000;
const CHECK_INTERVAL_MS = 6 * 60 * 60 * 1000; // 6 hours

export function useUpdateChecker() {
  const [updateInfo, setUpdateInfo] = useState<UpdateInfo | null>(null);
  const intervalRef = useRef<ReturnType<typeof setInterval>>(undefined);

  const doCheck = useCallback(async () => {
    try {
      const info = await checkForUpdate();
      if (info) {
        const dismissed = localStorage.getItem(DISMISSED_VERSION_KEY);
        if (dismissed === info.latest_version) {
          setUpdateInfo(null);
        } else {
          setUpdateInfo(info);
        }
      } else {
        setUpdateInfo(null);
      }
    } catch (e) {
      console.warn("Update check failed:", e);
    }
  }, []);

  useEffect(() => {
    const timeout = setTimeout(() => {
      doCheck();
      intervalRef.current = setInterval(doCheck, CHECK_INTERVAL_MS);
    }, INITIAL_DELAY_MS);

    return () => {
      clearTimeout(timeout);
      if (intervalRef.current) clearInterval(intervalRef.current);
    };
  }, [doCheck]);

  const dismissUpdate = useCallback(() => {
    if (updateInfo) {
      localStorage.setItem(DISMISSED_VERSION_KEY, updateInfo.latest_version);
      setUpdateInfo(null);
    }
  }, [updateInfo]);

  return { updateInfo, dismissUpdate };
}
