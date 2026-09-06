# Accessibility

What has been done for NFR-9, and — more usefully — what has not.

## Checked automatically

`npm run check:a11y` (`frontend/scripts/check-a11y.mjs`) runs over every component and
fails the build on:

- a control with no accessible name — an `<input>` or `<select>` without an `id` paired
  to a `<label htmlFor>`, an `aria-label`, or a wrapping `<label>`;
- a `role="progressbar"` missing `aria-valuenow`, `aria-valuemin` or `aria-valuemax`,
  which announces nothing;
- an empty `<button>`.

These are the failures that come back every time a component is edited, which is why
they are a check rather than a review note. It found one on first run: the
administrative-unit filter had no name.

## Contrast

Measured with the WCAG formula against both themes:

| | light | dark |
|---|---:|---:|
| body text on background | 17.22:1 | 15.07:1 |
| body text on panel | 15.68:1 | 13.69:1 |
| muted text on background | 6.39:1 | 7.06:1 |
| muted text on panel | 5.81:1 | 6.41:1 |
| accent text on panel | **4.53:1** | **4.52:1** |
| white on the primary button | 4.87:1 | 4.87:1 |

All clear AA (4.5:1) for normal text.

The swisstopo red `#DA291C` does **not**: as text it reaches only 4.43:1 on the light
panel and 3.32:1 on the dark one. Text therefore uses `--accent-text`, a variant of the
same hue adjusted per theme to clear 4.5:1, while the brand colour still fills buttons
and draws borders, where 3:1 is the threshold for graphical objects.

## Announcements

Build progress and the source download progress are `aria-live="polite"`, a finished
build is `role="status"`, and a failure is `role="alert"`. Without those a screen-reader
user sees a button that does nothing for two minutes.

## Keyboard and OS settings

- Every control is a real `<button>`, `<input>` or `<select>`, so tab order and
  activation come from the browser rather than from handlers.
- Selected state is carried by a border as well as colour (`aria-pressed` plus an inset
  outline), so it survives colour-blindness and a monochrome display.
- Focus is visible: `:focus-visible` draws a 2 px accent outline with an offset.
- `prefers-color-scheme` selects the theme; `prefers-reduced-motion` stops the
  indeterminate progress animation.
- Sizes are in `rem`, so OS text scaling works.

## Not done

- **No screen-reader pass.** Nobody has driven this with VoiceOver, NVDA or Orca. The
  static check proves controls *have* names; it cannot prove those names make sense read
  aloud, or that the reading order matches the visual one.
- **No keyboard-only walkthrough** of the whole wizard. The map in particular is drawn
  with a mouse: drawing a rectangle, a polygon or a circle has no keyboard equivalent.
  Every other way of choosing an area — place search, administrative units, a GPX
  corridor, whole Switzerland, an imported GeoJSON — is fully keyboard-operable, so the
  step is usable without a mouse, but one of its tools is not.
- **No testing at large text sizes** beyond checking that the units are relative.

The first two are the honest gaps. They need a person, not a script.
