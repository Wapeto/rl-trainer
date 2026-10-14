import { History, MessageSquare, PanelLeftClose, PanelLeftOpen, Settings, Target, TrendingUp, type LucideIcon } from "lucide-react";
import { LogoMark } from "@shared/LogoMark";

export type Page = "sessions" | "packs" | "progress" | "feedback" | "settings";

interface NavItem {
  id: Page;
  label: string;
  Icon: LucideIcon;
}

const NAV: NavItem[] = [
  { id: "sessions", label: "Sessions", Icon: History },
  { id: "packs", label: "Packs", Icon: Target },
  { id: "progress", label: "Progress", Icon: TrendingUp },
  { id: "settings", label: "Settings", Icon: Settings },
  // Last: pinned to the bottom, above the status pill.
  { id: "feedback", label: "Feedback", Icon: MessageSquare },
];

interface SidebarProps {
  page: Page;
  onPage: (page: Page) => void;
  folded: boolean;
  onFold: () => void;
  reviewCount: number;
  /** Ranked matches whose replay is still to save (spec §5.3). */
  toSaveCount: number;
  gameRunning: boolean;
  phoneConnected: boolean;
}

export const savePhrase = (n: number) => `${n} ${n === 1 ? "replay" : "replays"} to save`;

export function Sidebar({ page, onPage, folded, onFold, reviewCount, toSaveCount, gameRunning, phoneConnected }: SidebarProps) {
  const FoldIcon = folded ? PanelLeftOpen : PanelLeftClose;
  return (
    <aside className={`d-side${folded ? " d-side--rail" : ""}`} aria-label="Main">
      <div className="d-brand">
        <div className="d-logo">
          <LogoMark className="d-mark" />
          <span className="d-label">RL Trainer</span>
        </div>
        <button
          type="button"
          className="d-fold"
          aria-label={folded ? "Expand the sidebar" : "Collapse the sidebar"}
          onClick={onFold}
        >
          <FoldIcon className="d-icon" aria-hidden />
        </button>
      </div>
      {NAV.map(({ id, label, Icon }) => {
        const current = id === page;
        return (
          <button
            key={id}
            type="button"
            className={`d-nav${current ? " d-nav--on" : ""}${id === "feedback" ? " d-nav--foot" : ""}`}
            aria-current={current ? "page" : undefined}
            title={folded ? label : undefined}
            onClick={() => onPage(id)}
          >
            <Icon className="d-icon" aria-hidden />
            <span className="d-label">{label}</span>
            {id === "sessions" && reviewCount > 0 && (
              <span className="d-badge" aria-label={`${reviewCount} to review`}>
                {reviewCount}
              </span>
            )}
            {id === "sessions" && toSaveCount > 0 && (
              <span className="d-badge d-badge--blue" aria-label={savePhrase(toSaveCount)}>
                {toSaveCount}
              </span>
            )}
          </button>
        );
      })}
      <div className="d-health glass">
        <div title={gameRunning ? "Rocket League running" : "Rocket League closed"}>
          <span className={`d-led${gameRunning ? " d-led--on" : ""}`} aria-hidden />
          <span className="d-label">{gameRunning ? "Rocket League running" : "Rocket League closed"}</span>
        </div>
        <div title={phoneConnected ? "Phone connected" : "No phone"}>
          <span className={`d-led${phoneConnected ? " d-led--on" : ""}`} aria-hidden />
          <span className="d-label">{phoneConnected ? "Phone connected" : "No phone"}</span>
        </div>
      </div>
    </aside>
  );
}
