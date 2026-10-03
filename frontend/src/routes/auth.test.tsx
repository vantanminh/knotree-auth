import { render, screen } from "@testing-library/react";
import { StrictMode } from "react";
import { createMemoryRouter, RouterProvider } from "react-router";
import { ApiError, api } from "../lib/api";
import { HomePage, SignInPage, SignUpPage, VerifyEmailPage } from "./auth";

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
  expect(screen.getByLabelText("Email or username")).toBeInTheDocument();
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

it("keeps the service's authorize request when switching to sign-up", async () => {
  apiMock.mockReset();
  apiMock.mockImplementation((path: string) =>
    path.startsWith("/api/v1/oauth/context")
      ? Promise.resolve({ client_name: "Knotree Cloud" })
      : Promise.reject(new ApiError(401, "UNAUTHENTICATED", "Sign in required.")),
  );
  const returnTo = "/oauth/authorize?client_id=knotree-cloud&state=s";
  const router = createMemoryRouter(
    [
      { path: "/sign-in", element: <SignInPage /> },
      { path: "/sign-up", element: <SignUpPage /> },
    ],
    { initialEntries: [`/sign-up?return_to=${encodeURIComponent(returnTo)}`] },
  );
  render(<RouterProvider router={router} />);
  expect(await screen.findByText("Knotree Cloud")).toBeInTheDocument();
  expect(screen.getByRole("link", { name: "Sign in" })).toHaveAttribute(
    "href",
    `/sign-in?return_to=${encodeURIComponent(returnTo)}`,
  );
});

it("continues to the saved service after verifying the email", async () => {
  apiMock.mockReset();
  apiMock.mockResolvedValue({ status: "verified", return_to: "/oauth/authorize?client_id=knotree-cloud" });
  const router = createMemoryRouter([{ path: "/verify-email", element: <VerifyEmailPage /> }], {
    initialEntries: ["/verify-email?token=return-to-token"],
  });
  render(<RouterProvider router={router} />);
  const link = await screen.findByRole("link", { name: "Continue to sign in" });
  expect(link).toHaveAttribute(
    "href",
    `/sign-in?return_to=${encodeURIComponent("/oauth/authorize?client_id=knotree-cloud")}`,
  );
});
