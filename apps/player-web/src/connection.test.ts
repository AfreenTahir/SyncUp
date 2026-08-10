import { describe, expect, it } from "vitest";
import { parseServerEvent } from "./connection";

describe("WebSocket message parsing", () => {
  it("accepts known messages and rejects invalid data", () => {
    expect(parseServerEvent('{"type":"pong"}')).toEqual({ type: "pong" });
    expect(parseServerEvent('{"type":"error","code":"NOPE","message":"Bad"}')).toEqual({ type: "error", code: "NOPE", message: "Bad" });
    expect(parseServerEvent('{"type":"unknown"}')).toBeNull();
    expect(parseServerEvent("not json")).toBeNull();
  });
});

