import { render, screen } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { FIXTURE_NOW, fixtures } from "@shared/fixtures";
import { Freeplay } from "./Freeplay";

it("puts the hardest hit first, then touches and car-state shares", () => {
  const { container } = render(
    <Freeplay snap={fixtures.freeplay} conn="live" now={FIXTURE_NOW} api={null} notify={vi.fn()} />,
  );
  expect(container.firstChild).toHaveClass("tone-gold");
  expect(screen.getByText("Free play 8:32")).toBeInTheDocument();
  expect(container.querySelector(".hero__number")).toHaveTextContent("112km/h");
  expect(screen.getByText("Hardest hit")).toBeInTheDocument();
  expect(screen.getByText("Best today 118 km/h")).toBeInTheDocument();
  expect(screen.getByText("146")).toBeInTheDocument();
  expect(screen.getByText("23")).toBeInTheDocument();
  expect(screen.getByText("18%")).toBeInTheDocument();
  expect(screen.getByText("11%")).toBeInTheDocument();
  expect(screen.getByText("24%")).toBeInTheDocument();
});
