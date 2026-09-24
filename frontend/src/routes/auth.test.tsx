import { render, screen } from "@testing-library/react";
import { createMemoryRouter, RouterProvider } from "react-router";
import { SignInPage } from "./auth";

it("renders a focused sign-in form", () => {
  const router = createMemoryRouter([{ path: "/sign-in", element: <SignInPage /> }], {
    initialEntries: ["/sign-in"],
  });
  render(<RouterProvider router={router} />);
  expect(screen.getByRole("heading", { name: "Sign in" })).toBeInTheDocument();
  expect(screen.getByLabelText("Email")).toBeInTheDocument();
  expect(screen.getByLabelText("Password")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Continue" })).toBeInTheDocument();
});
