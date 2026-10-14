import {
  cloneElement,
  isValidElement,
  useCallback,
  useEffect,
  useId,
  useLayoutEffect,
  useRef,
  useState,
  type ReactElement,
  type ReactNode,
} from "react";
import { createPortal } from "react-dom";
import type { Gloss } from "../glossary";
import { place, type Side } from "./place";
import "./hint.css";

const OPEN_DELAY_MS = 150;

/** The open hint's close function: only one hint is open at a time. */
let closeCurrent: (() => void) | null = null;

const coarsePointer = () => window.matchMedia?.("(pointer: coarse)").matches ?? false;

interface HintProps {
  gloss: Gloss | null;
  children: ReactNode;
  /** The child is one focusable control (button, chip): no tab stop or underline here, its click stays its own. */
  control?: boolean;
  placement?: Side;
  className?: string;
}

interface Position {
  left: number;
  top: number;
  side: Side;
}

/** A label that explains itself: on hover or focus on desktop, on tap on the phone (spec §3.2). */
export function Hint({ gloss, children, control = false, placement = "below", className }: HintProps) {
  const id = useId();
  const anchor = useRef<HTMLSpanElement>(null);
  const tip = useRef<HTMLDivElement>(null);
  const timer = useRef<number | undefined>(undefined);
  /** A control was just pressed: its own focus must not reopen the hint over what the click opened. */
  const pressed = useRef(false);
  const [open, setOpen] = useState(false);
  const [pos, setPos] = useState<Position | null>(null);

  const close = useCallback(() => {
    window.clearTimeout(timer.current);
    setOpen(false);
    setPos(null);
  }, []);

  const show = useCallback(() => {
    window.clearTimeout(timer.current);
    if (closeCurrent !== close) closeCurrent?.();
    closeCurrent = close;
    setOpen(true);
  }, [close]);

  useEffect(
    () => () => {
      window.clearTimeout(timer.current);
      if (closeCurrent === close) closeCurrent = null;
    },
    [close],
  );

  useLayoutEffect(() => {
    if (!open) return;
    const measure = () => {
      const a = anchor.current?.getBoundingClientRect();
      const t = tip.current?.getBoundingClientRect();
      if (!a || !t) return;
      if (a.bottom < 0 || a.top > window.innerHeight) {
        close();
        return;
      }
      const view = { width: window.innerWidth, height: window.innerHeight };
      setPos(place(a, { width: t.width, height: t.height }, view, placement));
    };
    measure();
    window.addEventListener("scroll", measure, true);
    window.addEventListener("resize", measure);
    return () => {
      window.removeEventListener("scroll", measure, true);
      window.removeEventListener("resize", measure);
    };
  }, [open, placement, close]);

  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") close();
    };
    const onDown = (e: PointerEvent) => {
      const target = e.target as Node;
      if (!anchor.current?.contains(target) && !tip.current?.contains(target)) close();
    };
    document.addEventListener("keydown", onKey);
    document.addEventListener("pointerdown", onDown);
    return () => {
      document.removeEventListener("keydown", onKey);
      document.removeEventListener("pointerdown", onDown);
    };
  }, [open, close]);

  if (!gloss) return <>{children}</>;

  const describedBy = open ? id : undefined;
  const child =
    control && isValidElement(children)
      ? cloneElement(children as ReactElement<{ "aria-describedby"?: string }>, { "aria-describedby": describedBy })
      : children;
  const classes = ["hint", control ? "hint--control" : "", className ?? ""].filter(Boolean).join(" ");
  return (
    <>
      <span
        ref={anchor}
        className={classes}
        tabIndex={control ? undefined : 0}
        aria-describedby={control ? undefined : describedBy}
        onMouseEnter={() => {
          if (coarsePointer()) return;
          window.clearTimeout(timer.current);
          timer.current = window.setTimeout(show, OPEN_DELAY_MS);
        }}
        onMouseLeave={() => {
          pressed.current = false;
          if (!coarsePointer()) close();
        }}
        onPointerDown={() => {
          if (!control) return;
          pressed.current = true;
          close();
        }}
        onFocus={() => {
          if (!coarsePointer() && !pressed.current) show();
        }}
        onBlur={() => {
          if (!coarsePointer()) close();
        }}
        onClick={() => {
          if (!control && coarsePointer()) {
            if (open) close();
            else show();
          }
        }}
      >
        {child}
      </span>
      {open &&
        createPortal(
          <div
            ref={tip}
            id={id}
            role="tooltip"
            className={`hint-tip hint-tip--${pos?.side ?? placement}`}
            style={pos ? { left: pos.left, top: pos.top } : { left: 0, top: 0, visibility: "hidden" }}
          >
            <b>{gloss.title}</b>
            {gloss.text}
          </div>,
          document.body,
        )}
    </>
  );
}
