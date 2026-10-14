import { useCallback, useEffect, useState, useSyncExternalStore } from "react";
import { Toast, useToast } from "@shared/components/Toast";
import { useNow } from "@shared/hooks/useNow";
import { errorText, type DesktopApi, type UpdateInfo } from "./api";
import { browserStorage, readKey, writeKey, type KeyValueStore } from "./components/storage";
import { useLoad } from "./components/useLoad";
import type { SnapshotFeed } from "./live";
import type { PageProps } from "./pageProps";
import { reloadKey } from "./reloadKey";
import { PacksPage } from "./packs/PacksPage";
import { ProgressPage } from "./progress/ProgressPage";
import { SessionsPage } from "./sessions/SessionsPage";
import { FeedbackPage } from "./feedback/FeedbackPage";
import { SettingsPage } from "./settings/SettingsPage";
import { UpdateBanner } from "./UpdateBanner";
import { Sidebar, type Page } from "./Sidebar";

/** The first check waits, so the app's start is not slowed. */
export const UPDATE_CHECK_DELAY_MS = 10_000;
const FOLD_KEY = "rl-desktop-sidebar-folded";

export interface DesktopAppProps {
  api: DesktopApi;
  feed: SnapshotFeed;
  /** Fixed time for tests; the live clock otherwise. */
  now?: number;
  storage?: KeyValueStore | null;
}

export function DesktopApp({ api, feed, now: fixedNow, storage = browserStorage() }: DesktopAppProps) {
  const snapshot = useSyncExternalStore(feed.subscribe, feed.getState);
  const [page, setPage] = useState<Page>("sessions");
  const [focusId, setFocusId] = useState<number | null>(null);
  const [folded, setFolded] = useState(() => readKey(storage, FOLD_KEY) === "1");
  const [changes, setChanges] = useState(0);
  const ticking = useNow(30_000);
  const toast = useToast();
  const [update, setUpdate] = useState<UpdateInfo | null>(null);
  const reload = `${reloadKey(snapshot)}|${changes}`;
  const reviewCount = useLoad(() => api.reviewCount(), reload).data ?? 0;
  const toSaveCount = useLoad(() => api.replaysToSave(), reload).data?.length ?? 0;
  const onChanged = useCallback(() => setChanges((n) => n + 1), []);
  const goTo = useCallback((next: Page, id: number | null = null) => {
    setFocusId(id);
    setPage(next);
  }, []);

  const checkUpdate = useCallback(async () => {
    try {
      const found = await api.checkUpdate();
      setUpdate(found);
      return found;
    } catch (err) {
      console.warn("update check failed", errorText(err));
      throw err;
    }
  }, [api]);
  useEffect(() => {
    const id = setTimeout(() => void checkUpdate().catch(() => {}), UPDATE_CHECK_DELAY_MS);
    return () => clearTimeout(id);
  }, [checkUpdate]);
  async function checkNow() {
    try {
      const found = await checkUpdate();
      toast.show(found ? `Version ${found.version} is ready` : "You're up to date", "ok");
    } catch (err) {
      toast.show(`Couldn't check for updates: ${errorText(err)}`, "error");
    }
  }

  function fold() {
    const next = !folded;
    setFolded(next);
    writeKey(storage, FOLD_KEY, next ? "1" : "0");
  }

  const pageProps: PageProps = {
    api,
    snapshot,
    now: fixedNow ?? ticking,
    notify: toast.show,
    reload,
    onChanged,
    focusId,
    onCheckUpdate: checkNow,
    onOpenSettings: () => goTo("settings"),
  };
  return (
    <div className="d-app">
      <Sidebar
        page={page}
        onPage={(p) => goTo(p)}
        folded={folded}
        onFold={fold}
        reviewCount={reviewCount}
        toSaveCount={toSaveCount}
        gameRunning={snapshot?.game_running ?? false}
        phoneConnected={(snapshot?.phone_clients ?? 0) > 0}
      />
      {page === "sessions" && <SessionsPage {...pageProps} />}
      {page === "packs" && <PacksPage {...pageProps} />}
      {page === "progress" && (
        <ProgressPage
          {...pageProps}
          storage={storage}
          onOpenSession={(id) => goTo("sessions", id)}
          onOpenPack={(id) => goTo("packs", id)}
        />
      )}
      {page === "feedback" && <FeedbackPage {...pageProps} />}
      {page === "settings" && <SettingsPage {...pageProps} />}
      {update && (
        <UpdateBanner api={api} update={update} gameRunning={snapshot?.game_running ?? false} notify={toast.show} />
      )}
      <Toast toast={toast.current} />
    </div>
  );
}
