import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { Sidebar } from "./Sidebar";

function renderSidebar(folded = false) {
  return render(
    <Sidebar
      page="sessions"
      onPage={vi.fn()}
      folded={folded}
      onFold={vi.fn()}
      reviewCount={0}
      toSaveCount={0}
      gameRunning={false}
      phoneConnected={false}
    />,
  );
}

describe("Sidebar brand", () => {
  it("shows the RL Trainer logo mark next to the name", () => {
    const { container } = renderSidebar();
    const mark = container.querySelector(".d-logo svg.d-mark");
    expect(mark).not.toBeNull();
    expect(mark).toHaveAttribute("aria-hidden", "true");
    expect(screen.getByText("RL Trainer")).toBeInTheDocument();
  });

  it("keeps the logo mark when the sidebar is folded to a rail", () => {
    const { container } = renderSidebar(true);
    expect(container.querySelector(".d-side--rail .d-logo svg.d-mark")).not.toBeNull();
  });
});
