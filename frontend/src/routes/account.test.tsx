import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router";
import { api } from "../lib/api";
import { AuthorizedAppsPage } from "./account";

vi.mock("../lib/api", async () => {
  const actual = await vi.importActual<typeof import("../lib/api")>("../lib/api");
  return { ...actual, api: vi.fn() };
});

const apiMock = vi.mocked(api);

const study = {
  client_id: "knotree-study",
  name: "Knotree Study",
  first_party: true,
  status: "active",
  scopes: ["openid", "study:read"],
  granted_at: "2026-09-01T08:00:00Z",
  last_used_at: "2026-09-30T08:00:00Z",
  active_grants: 1,
};

afterEach(() => apiMock.mockReset());

it("lists the services the account has authorized", async () => {
  apiMock.mockResolvedValue({ items: [study] });
  render(
    <MemoryRouter>
      <AuthorizedAppsPage />
    </MemoryRouter>,
  );
  expect(await screen.findByText("Knotree Study")).toBeInTheDocument();
  expect(screen.getByText("study:read")).toBeInTheDocument();
  expect(screen.getByText("Knotree service")).toBeInTheDocument();
  expect(apiMock).toHaveBeenCalledWith("/api/v1/me/authorizations");
});

it("shows an empty state when nothing is authorized", async () => {
  apiMock.mockResolvedValue({ items: [] });
  render(
    <MemoryRouter>
      <AuthorizedAppsPage />
    </MemoryRouter>,
  );
  expect(await screen.findByText("No authorized apps")).toBeInTheDocument();
});

it("revokes access after confirmation", async () => {
  apiMock.mockResolvedValueOnce({ items: [study] });
  apiMock.mockResolvedValueOnce({ status: "revoked" });
  apiMock.mockResolvedValueOnce({ items: [] });
  render(
    <MemoryRouter>
      <AuthorizedAppsPage />
    </MemoryRouter>,
  );
  fireEvent.click(await screen.findByRole("button", { name: "Revoke access" }));
  const buttons = await screen.findAllByRole("button", { name: "Revoke access" });
  fireEvent.click(buttons[buttons.length - 1]);
  await waitFor(() =>
    expect(apiMock).toHaveBeenCalledWith("/api/v1/me/authorizations/knotree-study", { method: "DELETE" }),
  );
  expect(await screen.findByText("No authorized apps")).toBeInTheDocument();
});
