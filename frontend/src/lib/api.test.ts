import { safeReturnTo } from "./api";

describe("safeReturnTo", () => {
  it("allows an authorize continuation", () => {
    expect(safeReturnTo("/oauth/authorize?client_id=knotree-study")).toBe("/oauth/authorize?client_id=knotree-study");
  });

  it("rejects open redirects", () => {
    expect(safeReturnTo("https://attacker.example")).toBeNull();
    expect(safeReturnTo("//attacker.example")).toBeNull();
    expect(safeReturnTo("/\\attacker")).toBeNull();
    expect(safeReturnTo("/oauth/../admin")).toBeNull();
  });
});
