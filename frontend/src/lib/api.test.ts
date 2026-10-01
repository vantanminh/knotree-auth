import { destinationAfterAuth, safeReturnTo } from "./api";

describe("safeReturnTo", () => {
  it("allows an authorize continuation", () => {
    expect(safeReturnTo("/oauth/authorize?client_id=knotree-study")).toBe("/oauth/authorize?client_id=knotree-study");
  });

  it("sends an existing session to the account page", () => {
    expect(destinationAfterAuth(null)).toBe("/account");
    expect(destinationAfterAuth("/sign-in")).toBe("/account");
    expect(destinationAfterAuth("/account/security")).toBe("/account/security");
    expect(destinationAfterAuth("/oauth/authorize?client_id=knotree-study")).toBe(
      "/oauth/authorize?client_id=knotree-study",
    );
  });

  it("rejects open redirects", () => {
    expect(safeReturnTo("https://attacker.example")).toBeNull();
    expect(safeReturnTo("//attacker.example")).toBeNull();
    expect(safeReturnTo("/\\attacker")).toBeNull();
    expect(safeReturnTo("/oauth/../admin")).toBeNull();
  });
});
