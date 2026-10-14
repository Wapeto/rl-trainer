import { describe, expect, it, vi } from "vitest";
import { ApiError, createApi } from "./client";

function respond(body: unknown, status = 200) {
  return vi.fn().mockResolvedValue(new Response(JSON.stringify(body), { status }));
}

describe("createApi", () => {
  it("asks the PC for packs matching a search", async () => {
    const fetchImpl = vi.fn().mockResolvedValue(new Response(JSON.stringify({ ok: true, data: [] })));
    await createApi("tok", fetchImpl).packs("c7e0 9e");
    expect(String(fetchImpl.mock.calls[0][0])).toContain("/api/packs?search=c7e0%209e");
  });

  it("sends the bearer token and JSON body", async () => {
    const fetchImpl = respond({ ok: true, data: { block_id: 40, pack_id: 11 } });
    await createApi("tok", fetchImpl).setPack(40, 11);
    const [path, init] = fetchImpl.mock.calls[0];
    expect(path).toBe("/api/blocks/40/pack");
    expect(init.method).toBe("PUT");
    expect(init.headers.Authorization).toBe("Bearer tok");
    expect(JSON.parse(init.body)).toEqual({ pack_id: 11 });
  });

  it("turns the error envelope into an ApiError with the server's message", async () => {
    const fetchImpl = respond(
      { ok: false, error: { code: "invalid", message: "MMR must be between 0 and 3000" } },
      422,
    );
    const err = await createApi("tok", fetchImpl).reviewSession(7, [], null).catch((e) => e);
    expect(err).toBeInstanceOf(ApiError);
    expect([err.code, err.status, err.message]).toEqual(["invalid", 422, "MMR must be between 0 and 3000"]);
  });

  it("reports an unreachable PC plainly", async () => {
    const fetchImpl = vi.fn().mockRejectedValue(new TypeError("Failed to fetch"));
    const err = await createApi("tok", fetchImpl).packs().catch((e) => e);
    expect([err.code, err.message]).toEqual(["network", "Can't reach the PC. Check the Wi-Fi."]);
  });

  it("reports a refused token so the page can ask to pair again", async () => {
    const fetchImpl = respond({ ok: false, error: { code: "unauthorized", message: "scan the QR code again" } }, 401);
    const onUnauthorized = vi.fn();
    await createApi("tok", fetchImpl, onUnauthorized).packs().catch(() => undefined);
    expect(onUnauthorized).toHaveBeenCalledTimes(1);
  });
});
