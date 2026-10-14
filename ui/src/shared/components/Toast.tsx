import { useCallback, useEffect, useState } from "react";

export type Notify = (text: string, tone?: "error" | "ok") => void;

export interface ToastMessage {
  id: number;
  text: string;
  tone: "error" | "ok";
}

let nextId = 1;

export function useToast(timeoutMs = 3000) {
  const [current, setCurrent] = useState<ToastMessage | null>(null);
  const show = useCallback<Notify>((text, tone = "error") => {
    setCurrent({ id: nextId++, text, tone });
  }, []);
  useEffect(() => {
    if (!current) return;
    const id = setTimeout(() => setCurrent(null), timeoutMs);
    return () => clearTimeout(id);
  }, [current, timeoutMs]);
  return { current, show };
}

export function Toast({ toast }: { toast: ToastMessage | null }) {
  return (
    <div className="toast-zone">
      {toast && (
        <div key={toast.id} className={`toast glass toast--${toast.tone}`} role={toast.tone === "error" ? "alert" : "status"}>
          {toast.text}
        </div>
      )}
    </div>
  );
}
