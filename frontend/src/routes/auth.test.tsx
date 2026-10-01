import { render, screen } from "@testing-library/react";
import { createMemoryRouter, RouterProvider } from "react-router";
import { ApiError, api } from "../lib/api";
import { HomePage, SignInPage } from "./auth";

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
