import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import "@fontsource/manrope/400.css";
import "@fontsource/manrope/500.css";
import "@fontsource/manrope/600.css";
import "@fontsource/manrope/700.css";
import "@fontsource/manrope/800.css";
import "@shared/tokens.css";
import "@shared/components/components.css";
import "./desktop.css";
import { createDesktopApi, httpTransport, tauriGlobal, tauriTransport } from "./api";
import { DesktopApp } from "./App";
import { LiveFeed, followTauri, poll } from "./live";

const tauri = tauriGlobal();
const api = createDesktopApi(tauri ? tauriTransport(tauri) : httpTransport());
const feed = new LiveFeed();
if (tauri) {
  // Without the event (should not happen), polling still keeps the window current.
  followTauri(feed, tauri, api).catch(() => poll(feed, api));
} else {
  poll(feed, api);
}

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <DesktopApp api={api} feed={feed} />
  </StrictMode>,
);
