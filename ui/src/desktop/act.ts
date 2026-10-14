import type { Notify } from "@shared/components/Toast";
import { errorText } from "./api";

export const sentence = (text: string) => (text ? text[0].toUpperCase() + text.slice(1) : text);

/** Runs an action, reports a failure in the toast (and success when `okText` is given). */
export async function act(notify: Notify, action: () => Promise<unknown>, okText?: string): Promise<boolean> {
  try {
    await action();
    if (okText) notify(okText, "ok");
    return true;
  } catch (err) {
    notify(sentence(errorText(err)), "error");
    return false;
  }
}
