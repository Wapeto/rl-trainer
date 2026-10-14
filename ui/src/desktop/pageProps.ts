import type { Notify } from "@shared/components/Toast";
import type { AppSnapshot } from "@shared/generated/types";
import type { DesktopApi } from "./api";

export interface PageProps {
  api: DesktopApi;
  snapshot: AppSnapshot | null;
  now: number;
  notify: Notify;
  /** Changes whenever what the page shows may be stale; pages reload on it. */
  reload: string;
  /** Call after an edit so the badge and the other pages catch up. */
  onChanged: () => void;
  /** The item to open with (a click on a Progress point or "Open in Packs"). */
  focusId?: number | null;
  /** Settings → About → Check for updates; shows the update banner when there is one. */
  onCheckUpdate?: () => Promise<void>;
  /** Opens Settings (the Get started card's links). */
  onOpenSettings?: () => void;
}
