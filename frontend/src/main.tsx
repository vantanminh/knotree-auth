import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { createBrowserRouter, Navigate, RouterProvider } from "react-router";
import { AccountShell, AdminShell } from "./components/shells";
import {
  AccountHome,
  ConnectedAccountsPage,
  ProfilePage,
  SecurityPage,
  SessionsPage,
} from "./routes/account";
import { AdminLogs, AdminOverview, AdminSecurity, AdminUser, AdminUsers } from "./routes/admin";
import { ForgotPasswordPage, ResetPasswordPage, SignInPage, SignUpPage, VerifyEmailPage } from "./routes/auth";
import { MfaPage } from "./routes/mfa";
import { ConsentPage, OAuthErrorPage } from "./routes/oauth";
import "./styles.css";

const router = createBrowserRouter([
  { path: "/", element: <Navigate to="/sign-in" replace /> },
  { path: "/sign-in", element: <SignInPage /> },
  { path: "/sign-up", element: <SignUpPage /> },
  { path: "/verify-email", element: <VerifyEmailPage /> },
  { path: "/forgot-password", element: <ForgotPasswordPage /> },
  { path: "/reset-password", element: <ResetPasswordPage /> },
  { path: "/mfa", element: <Navigate to="/mfa/totp" replace /> },
  { path: "/mfa/totp", element: <MfaPage method="totp" /> },
  { path: "/mfa/email", element: <MfaPage method="email" /> },
  { path: "/mfa/recovery", element: <MfaPage method="recovery" /> },
  { path: "/oauth/consent", element: <ConsentPage /> },
  { path: "/oauth/error", element: <OAuthErrorPage /> },
  {
    path: "/account",
    element: <AccountShell />,
    children: [
      { index: true, element: <AccountHome /> },
      { path: "profile", element: <ProfilePage /> },
      { path: "security", element: <SecurityPage /> },
      { path: "sessions", element: <SessionsPage /> },
      { path: "connected-accounts", element: <ConnectedAccountsPage /> },
    ],
  },
  {
    path: "/admin",
    element: <AdminShell />,
    children: [
      { index: true, element: <AdminOverview /> },
      { path: "users", element: <AdminUsers /> },
      { path: "users/:id", element: <AdminUser /> },
      { path: "security", element: <AdminSecurity /> },
      { path: "logs", element: <AdminLogs /> },
    ],
  },
]);

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <RouterProvider router={router} />
  </StrictMode>,
);
