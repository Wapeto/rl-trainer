import { describe, expect, it } from "vitest";
import { codeProblem, formatCode } from "./packCode";

describe("pack codes", () => {
  it("formats what is typed: upper case, a dash every four characters", () => {
    expect(formatCode("c7e09e0bb739a899")).toBe("C7E0-9E0B-B739-A899");
    expect(formatCode(" c7e0 9e0b-b7")).toBe("C7E0-9E0B-B7");
    expect(formatCode("")).toBe("");
  });

  it("says why a code is not valid yet", () => {
    expect(codeProblem("C7E0-9E0B")).toBe("8 of 16 characters");
    expect(codeProblem("C7E0-9E0B-B739-A8991")).toBe("17 of 16 characters");
    expect(codeProblem("C7E0-9E0B-B739-A8O9")).toBe("Only 0–9 and A–F");
    expect(codeProblem("c7e09e0bb739a899")).toBeNull();
  });
});
