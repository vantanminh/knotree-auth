# Frontend design system

The accounts UI is a utility surface. It should feel calm, complete, and specific, closer to a bank settings page than a product launch. No gradients, glass effects, or decorative illustrations.

## Foundations

Tokens live in `frontend/src/styles.css` (`@theme`).

- Paper `#f6f5f2`, surface `#fffdfa`, sunken `#efede8`, ink `#1c1917`, muted `#6b6560`, faint `#9a948d`, line `#e4e0d8` / `#d3cec4`.
- Pine `#1f3d32` is the only accent; danger `#8f2d2d` and amber `#8a5a12` mark risk and warnings. Each has a `-soft` fill and `-line` border.
- Be Vietnam Pro for text (Vietnamese + Latin subsets; designed for stacked Vietnamese diacritics), IBM Plex Mono for codes, IDs, IPs, and setup keys. Numbers in tables and metrics use tabular figures.
- 24px page titles, 15px card titles, 14–15px body. 6px control radius, 10px card radius, 14px auth card radius.
- Shadows are hairline (`shadow-card`); `shadow-raised` for the auth card, `shadow-overlay` for dialogs.
- Motion: color transitions ~150ms, a 220ms fade-up when a page or panel appears. All disabled under `prefers-reduced-motion`.

## Layout

- Auth pages: logo top-left, one centered 400px card, secondary links below the card, a quiet footer.
- Account and admin: 256px sidebar with icon navigation and the signed-in user at the bottom; on mobile a sticky top bar with scrollable tabs. Content is at most 880px wide.
- Settings are grouped in cards: header (title + one-line description), body, footer with a short note on the left and the action on the right.
- Lists inside cards use full-bleed dividers. Rows put an icon tile, title, and description on the left and the action on the right.

## Controls

All primitives are in `frontend/src/components/ui.tsx` on top of Base UI: `Button` (primary, secondary, ghost, danger, danger-solid; sm/md/lg), `TextField` (password reveal, inline error), `CodeField`, `Switch`, `Checkbox`, `Badge`, `Alert`, `Card`, `Row`, `ConfirmDialog`, `CopyButton`, `Skeleton`, `EmptyState`. Icons are inline SVG in `components/icons.tsx` (20px grid, 1.6 stroke).

Destructive or irreversible actions (delete account, revoke sessions, regenerate codes, admin actions) always go through `ConfirmDialog`, never `window.confirm`.

One-time codes use a single input with `autocomplete=one-time-code`, not six boxes.

## Copy

Use “Sign in”, “Create account”, “Verify your email”, “Enter verification code”. Do not add marketing headlines.

## States

- Pending buttons show a spinner and a progressive verb (“Signing in…”) without changing size.
- Loading pages show skeletons in the shape of the content, not “Loading…” text.
- Errors and confirmations appear inside the card they belong to, as an `Alert`.
- Empty lists use `EmptyState`: an icon, a title, and one sentence saying what is missing.

## Loading feedback

- Buttons lock on the first click (`pending`), but the spinner appears only after 150 ms and then stays at least 400 ms (`useDelayedBusy` in `src/lib/loading.ts`), so fast actions never flicker.
- Every `api()` call is tracked; a 2px pine progress bar (`TopProgress`) shows at the top of the page while any request takes longer than 200 ms.
- Page skeletons fade in after 120 ms so instant loads go straight to content. Service logos shimmer until the image has loaded, then fade in.
- All motion is disabled under `prefers-reduced-motion: reduce`.
- Service logos (`ClientLogo`) are square images shown with rounded corners (22% of the size), falling back to initials.
