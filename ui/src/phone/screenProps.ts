import type { Api } from "@shared/api/client";
import type { ConnState } from "@shared/api/liveStore";
import type { Notify } from "@shared/components/Toast";
import type { AppSnapshot } from "@shared/generated/types";

export interface ScreenProps {
  snap: AppSnapshot;
  conn: ConnState;
  now: number;
  api: Api | null;
  notify: Notify;
}
