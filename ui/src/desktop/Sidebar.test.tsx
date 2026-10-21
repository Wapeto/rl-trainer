import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { Sidebar } from "./Sidebar";

const onPairPhone = vi.fn();

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
      onPairPhone={onPairPhone}
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

  it("opens the pairing dialog from the phone line", async () => {
    renderSidebar();
    await userEvent.click(screen.getByRole("button", { name: "Pair a phone" }));
    expect(onPairPhone).toHaveBeenCalledTimes(1);
  });
});
