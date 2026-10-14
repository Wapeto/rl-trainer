import { ApiError } from "@shared/api/client";
import type { Notify } from "@shared/components/Toast";

export const sentence = (text: string) => (text ? text[0].toUpperCase() + text.slice(1) : text);

/** Runs an API action and reports the outcome in the toast. No optimistic update: the next
 * snapshot shows the change. */
export async function act(notify: Notify, action: () => Promise<unknown>, okText: string): Promise<boolean> {
  try {
    await action();
    notify(okText, "ok");
    return true;
  } catch (err) {
    notify(err instanceof ApiError ? sentence(err.message) : "Something went wrong on the PC.", "error");
    return false;
  }
}
