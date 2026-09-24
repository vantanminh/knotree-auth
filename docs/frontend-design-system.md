# Frontend design system

The accounts UI is a utility surface. It should feel calm and specific, closer to a bank settings page than a product launch.

## Foundations

- Paper `#f6f5f2`, surface `#fffcf8`, ink `#1c1917`, muted `#6b6560`, line `#e4e0d8`, pine `#1f3d32`, danger `#8f2d2d`.
- IBM Plex Sans, 16px body, 22px page titles, 6px control radius.
- Auth forms are 360px wide. Admin and account navigation is 216px.
- Motion is color only, about 150ms, and disabled under `prefers-reduced-motion`.

## Controls

Buttons, fields, and labels use Base UI primitives styled in `frontend/src/components/ui.tsx`. Primary buttons are pine. Quiet buttons are bordered. Danger is used for disable and delete.

One-time codes use a single input with `autocomplete=one-time-code`, not six boxes.

## Copy

Use “Sign in”, “Create account”, “Verify your email”, “Enter verification code”. Do not add marketing headlines.

## States

Submit labels change to a progressive verb (“Signing in…”) without changing the button size. Errors sit above the form in an alert. Empty admin and activity lists say what is missing in one sentence.
