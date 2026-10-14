import { useEffect, useRef, useState } from "react";
import { errorText } from "../api";

export interface Loaded<T> {
  /** Undefined until the first answer; kept while a reload runs (no flicker). */
  data: T | undefined;
  error: string | null;
  loading: boolean;
}

/** Loads whenever `key` changes; a late answer for an old key is dropped. */
export function useLoad<T>(load: () => Promise<T>, key: string): Loaded<T> {
  const [state, setState] = useState<Loaded<T>>({ data: undefined, error: null, loading: true });
  const latest = useRef(load);
  latest.current = load;
  useEffect(() => {
    let current = true;
    setState((s) => ({ ...s, loading: true }));
    latest.current().then(
      (data) => current && setState({ data, error: null, loading: false }),
      (err: unknown) => current && setState((s) => ({ data: s.data, error: errorText(err), loading: false })),
    );
    return () => {
      current = false;
    };
  }, [key]);
  return state;
}

export function useDebounced<T>(value: T, ms: number): T {
  const [settled, setSettled] = useState(value);
  useEffect(() => {
    const id = setTimeout(() => setSettled(value), ms);
    return () => clearTimeout(id);
  }, [value, ms]);
  return settled;
}
