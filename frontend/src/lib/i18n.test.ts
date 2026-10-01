import { detectLocale, getLocale, parseLocale, setLocale, t } from "./i18n";

describe("i18n", () => {
  afterEach(() => setLocale("en"));

  it("parses supported language tags", () => {
    expect(parseLocale("vi-VN")).toBe("vi");
    expect(parseLocale("en_US")).toBe("en");
    expect(parseLocale("fr")).toBeNull();
  });

  it("prefers the saved choice over the browser language", () => {
    setLocale("vi");
    expect(detectLocale()).toBe("vi");
    expect(document.documentElement.lang).toBe("vi");
  });

  it("translates and interpolates", () => {
    setLocale("vi");
    expect(getLocale()).toBe("vi");
    expect(t("Sign in")).toBe("Đăng nhập");
    expect(t("Resend in {seconds}s", { seconds: 30 })).toBe("Gửi lại sau 30s");
    expect(t("Untranslated text")).toBe("Untranslated text");
    setLocale("en");
    expect(t("Sign in")).toBe("Sign in");
  });
});
