import { act, renderHook } from "@testing-library/react";
import { useDelayedBusy } from "./loading";

describe("useDelayedBusy", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it("never shows for fast work", () => {
    const { result, rerender } = renderHook(({ busy }) => useDelayedBusy(busy), { initialProps: { busy: true } });
    act(() => vi.advanceTimersByTime(100));
    rerender({ busy: false });
    act(() => vi.advanceTimersByTime(500));
    expect(result.current).toBe(false);
  });

  it("shows for slow work and stays up long enough not to flicker", () => {
    const { result, rerender } = renderHook(({ busy }) => useDelayedBusy(busy), { initialProps: { busy: true } });
    act(() => vi.advanceTimersByTime(160));
    expect(result.current).toBe(true);
    rerender({ busy: false });
    act(() => vi.advanceTimersByTime(100));
    expect(result.current).toBe(true);
    act(() => vi.advanceTimersByTime(400));
    expect(result.current).toBe(false);
  });
});
