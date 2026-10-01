import { render, screen } from "@testing-library/react";
import { StrictMode } from "react";
import { createMemoryRouter, RouterProvider } from "react-router";
import { ApiError, api } from "../lib/api";
import { HomePage, SignInPage, VerifyEmailPage } from "./auth";

vi.mock("../lib/api", async () => {
  const actual = await vi.importActual<typeof import("../lib/api")>("../lib/api");
  return { ...actual, api: vi.fn() };
});

const apiMock = vi.mocked(api);

function renderAt(path: string) {
  const router = createMemoryRouter(
    [
      { path: "/", element: <HomePage /> },
      { path: "/sign-in", element: <SignInPage /> },
      { path: "/account", element: <h1>Account</h1> },
    ],
    { initialEntries: [path] },
  );
  render(<RouterProvider router={router} />);
}

it("renders a focused sign-in form when the browser has no session", async () => {
  apiMock.mockRejectedValue(new ApiError(401, "UNAUTHENTICATED", "Sign in required."));
  renderAt("/sign-in");
  expect(await screen.findByRole("heading", { name: "Sign in" })).toBeInTheDocument();
  expect(screen.getByLabelText("Email")).toBeInTheDocument();
  expect(screen.getByLabelText("Password")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Continue" })).toBeInTheDocument();
});

it("opens the account page when a session is still valid", async () => {
  apiMock.mockResolvedValue({ email: "ada@example.com" });
  renderAt("/sign-in");
  expect(await screen.findByRole("heading", { name: "Account" })).toBeInTheDocument();
  expect(screen.queryByRole("heading", { name: "Sign in" })).not.toBeInTheDocument();
});

it("resumes the account page from the site root", async () => {
  apiMock.mockResolvedValue({ email: "ada@example.com" });
  renderAt("/");
  expect(await screen.findByRole("heading", { name: "Account" })).toBeInTheDocument();
});

it("sends a single verify request per token under StrictMode", async () => {
  apiMock.mockReset();
  apiMock.mockResolvedValueOnce({}).mockRejectedValue(new ApiError(400, "INVALID_TOKEN", "This link is not valid."));
  const router = createMemoryRouter([{ path: "/verify-email", element: <VerifyEmailPage /> }], {
    initialEntries: ["/verify-email?token=strict-mode-token"],
  });
  render(
    <StrictMode>
      <RouterProvider router={router} />
    </StrictMode>,
  );
  expect(await screen.findByText("Email verified. You can sign in.")).toBeInTheDocument();
  const verifyCalls = apiMock.mock.calls.filter(([path]) => path === "/api/v1/auth/email/verify");
  expect(verifyCalls).toHaveLength(1);
});
