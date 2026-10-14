import { describe, expect, it } from "vitest";
import { fixtures } from "../fixtures";
import { LiveStore, RETRY_DELAYS_MS, UNAUTHORIZED } from "./liveStore";

class FakeSocket {
  sent: string[] = [];
  closed = false;
  onopen: ((ev: Event) => void) | null = null;
  onmessage: ((ev: MessageEvent) => void) | null = null;
  onclose: ((ev: CloseEvent) => void) | null = null;
  send(data: string) {
    this.sent.push(data);
  }
  close() {
    this.closed = true;
  }
  open() {
    this.onopen?.(new Event("open"));
  }
  message(data: unknown) {
    this.onmessage?.({ data: JSON.stringify(data) } as MessageEvent);
  }
  drop(code = 1006) {
    this.onclose?.({ code } as CloseEvent);
  }
}

function harness(token: string | null = "tok") {
  const sockets: FakeSocket[] = [];
  const delays: number[] = [];
  let pending: (() => void) | null = null;
  const store = new LiveStore({
    token,
    url: "ws://pc/api/live/ws",
    connect: () => {
      const s = new FakeSocket();
      sockets.push(s);
      return s;
    },
    schedule: (fn, ms) => {
      delays.push(ms);
      pending = fn;
    },
  });
  return { store, sockets, delays, retry: () => pending?.() };
}

describe("LiveStore", () => {
  it("authenticates first, then applies snapshots", () => {
    const { store, sockets } = harness();
    store.start();
    sockets[0].open();
    expect(JSON.parse(sockets[0].sent[0])).toEqual({ type: "auth", token: "tok" });
    sockets[0].message({ type: "snapshot", data: fixtures.idle });
    expect(store.getState()).toEqual({ conn: "live", snapshot: fixtures.idle });
  });

  it("keeps the last snapshot while reconnecting with backoff", () => {
    const { store, sockets, delays, retry } = harness();
    store.start();
    sockets[0].message({ type: "snapshot", data: fixtures.idle });
    sockets[0].drop();
    expect(store.getState()).toEqual({ conn: "reconnecting", snapshot: fixtures.idle });
    retry();
    sockets[1].drop();
    retry();
    sockets[2].drop();
    retry();
    sockets[3].drop();
    expect(delays).toEqual([...RETRY_DELAYS_MS, 5000]);
    retry();
    sockets[4].message({ type: "snapshot", data: fixtures.trainingBelow });
    expect(store.getState().conn).toBe("live");
    sockets[4].drop();
    expect(delays.at(-1)).toBe(1000);
  });

  it("stops and asks for pairing when the token is refused", () => {
    const { store, sockets, delays } = harness();
    store.start();
    sockets[0].drop(UNAUTHORIZED);
    expect(store.getState().conn).toBe("unpaired");
    expect(delays).toEqual([]);
  });

  it("is unpaired without a token and never connects", () => {
    const { store, sockets } = harness(null);
    store.start();
    expect(store.getState()).toEqual({ conn: "unpaired", snapshot: null });
    expect(sockets).toHaveLength(0);
  });

  it("reconnects at once on resume, keeping the last snapshot", () => {
    // After the phone sleeps the old socket may be half-open: it never reports a close.
    const { store, sockets, delays } = harness();
    store.start();
    sockets[0].message({ type: "snapshot", data: fixtures.idle });
    store.resume();
    expect(sockets[0].closed).toBe(true);
    expect(sockets).toHaveLength(2);
    sockets[0].drop(); // a late close from the old socket schedules nothing
    expect(delays).toEqual([]);
    expect(store.getState().snapshot).toEqual(fixtures.idle);
    sockets[1].open();
    expect(JSON.parse(sockets[1].sent[0])).toEqual({ type: "auth", token: "tok" });
  });

  it("does not resume when unpaired or stopped", () => {
    const unpaired = harness(null);
    unpaired.store.resume();
    expect(unpaired.sockets).toHaveLength(0);
    const { store, sockets } = harness();
    store.start();
    store.stop();
    store.resume();
    expect(sockets).toHaveLength(1);
  });

  it("notifies subscribers", () => {
    const { store, sockets } = harness();
    let calls = 0;
    const unsubscribe = store.subscribe(() => calls++);
    store.start();
    sockets[0].message({ type: "snapshot", data: fixtures.idle });
    unsubscribe();
    sockets[0].message({ type: "snapshot", data: fixtures.idle });
    expect(calls).toBe(1);
  });
});

describe("markUnpaired", () => {
  it("shows the pairing screen and stops reconnecting", () => {
    const socket = new FakeSocket();
    const scheduled: (() => void)[] = [];
    const store = new LiveStore({ token: "t", connect: () => socket, schedule: (fn) => void scheduled.push(fn) });
    store.start();
    socket.open();
    store.markUnpaired();
    expect(store.getState().conn).toBe("unpaired");
    expect(socket.closed).toBe(true);
    socket.drop();
    expect(scheduled).toHaveLength(0);
  });
});
