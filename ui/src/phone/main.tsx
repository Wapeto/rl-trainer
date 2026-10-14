import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import "@fontsource/manrope/300.css";
import "@fontsource/manrope/400.css";
import "@fontsource/manrope/500.css";
import "@fontsource/manrope/600.css";
import "@fontsource/manrope/700.css";
import "@fontsource/manrope/800.css";
import "@shared/tokens.css";
import "@shared/components/components.css";
import "./phone.css";
import { createApi } from "@shared/api/client";
import { LiveStore } from "@shared/api/liveStore";
import { KeepAwake, browserKeepAwakeDeps } from "@shared/keepAwake";
import { takeToken } from "@shared/token";
import { PhoneApp } from "./App";

const token = takeToken();
const store = new LiveStore({ token });
const keepAwake = new KeepAwake(browserKeepAwakeDeps());
store.start();
// A sleeping phone can leave the socket half-open without a close event: reconnect on return.
document.addEventListener("visibilitychange", () => {
  if (document.visibilityState === "visible") store.resume();
});
window.addEventListener("online", () => store.resume());

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <PhoneApp source={store} api={token ? createApi(token, undefined, () => store.markUnpaired()) : null} keepAwake={keepAwake} />
  </StrictMode>,
);
