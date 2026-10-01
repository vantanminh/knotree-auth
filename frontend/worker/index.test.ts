import worker from "./index";

afterEach(() => {
  vi.unstubAllGlobals();
});

it("forwards the session cookie and does not cache API responses", async () => {
  const upstream = vi.fn(async (input: Request | URL, init?: RequestInit) => {
    const request = input instanceof Request ? input : new Request(input, init);
    expect(request.headers.get("cookie")).toBe("knotree_session=abc");
    expect(request.url).toBe("https://accounts-api.knotree.com/api/v1/me");
    expect(request.cache).toBe("no-store");
    const headers = new Headers();
    headers.append("set-cookie", "knotree_session=abc; Max-Age=1209600; Path=/; HttpOnly");
    headers.append("set-cookie", "knotree_csrf=csrf; Max-Age=86400; Path=/");
    return new Response("{}", { status: 200, headers });
  });
  vi.stubGlobal("fetch", upstream);

  const response = await worker.fetch(
    new Request("https://accounts.knotree.com/api/v1/me", {
      headers: { cookie: "knotree_session=abc" },
    }),
    {
      ASSETS: { fetch: async () => new Response("asset") },
      API_ORIGIN: "https://accounts-api.knotree.com",
    },
  );

  expect(upstream).toHaveBeenCalledOnce();
  expect(response.status).toBe(200);
  expect(response.headers.get("cache-control")).toBe("no-store");
  const cookies = response.headers.getSetCookie();
  expect(cookies.some((cookie) => cookie.startsWith("knotree_session=abc"))).toBe(true);
  expect(cookies.some((cookie) => cookie.startsWith("knotree_csrf=csrf"))).toBe(true);
});
