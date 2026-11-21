# Style and UX audit

Scorpio's settings window, checked against the owner's rules and the
lxxvii standard style (`~/.claude/skills/lxxvii-style`, published in the
private LXXVII-Design repository). Written 2026-11-21.

## Rules from the owner

| Rule | How Scorpio meets it | Proof |
| --- | --- | --- |
| No checkboxes | Every on/off is a `.switch` (`role="switch"`). Choices of two or three are segmented controls, longer ones drawn dropdowns. The page has no `<input type="checkbox">` or `type="radio"` | `grep -n 'checkbox\|radio"' ui/*.js ui/*.html` finds only `role="radio"` on segmented buttons |
| No square next to rounded | One shape language: the kit's 2px cut-stone corner on every control and container, 1px on parts under 20px, no pills or discs. The only round things are status points of 10px or less | `ui/components.css` header; `border-radius` values in `ui/app.css` are none |
| Remove what doesn't earn its place | Theme choice, image presets, custom images, tint, reticle, app size, sounds, follow mouse, tilt, duplicates, update checks: gone (see the plan) | `docs/plans/scorpio.md` |
| Logical, obvious UX | See below | Screenshots of every tab, rendered headless with a mocked IPC |

## Decisions

- **Four tabs, not six.** The plan listed Crosshair, Games, Actions,
  Keybinds, System, About. Actions held one setting (hide while aiming) and
  About one paragraph, so Actions and Keybinds became **Controls**, and
  About became the foot of **System**. Fewer places to look.
- **The crosshair is edited where it is seen.** The preview sits on the
  left at actual size (SVG units are physical pixels, divided by the
  device pixel ratio), with a 4× view for detail. Controls scroll beside
  it; the preview stays.
- **One editor for every crosshair.** A game with its own crosshair is
  edited in the same Crosshair tab, chosen in a dropdown above the
  preview that appears only when such a game exists. No second editor, no
  extra layer of windows (the owner's complaint about CrossOver).
- **Share codes are one field.** Paste to import, Copy to export. The
  outline width is kept on import, because CS2 codes don't carry it, and
  the help line says so.
- **Saved crosshairs are tiles.** Click applies; the picked tile shows
  rename, Update and Delete beneath the grid. The next and previous
  shortcuts walk the same list.
- **Status lives in the title bar.** One line says what the overlay is
  doing now: shown over which game, hidden until a game is in front,
  hidden by shortcut, or unlocked for moving.
- **Shortcuts can't steal game keys.** A shortcut needs Ctrl, Alt or
  Super unless it is an F-key. A shortcut another program holds is
  marked and explained.
- **Platform honesty.** "Hide from screen capture" is disabled on Linux
  with the reason, instead of a switch that does nothing.
- **Motion.** Only the kit's state changes (colour .3s, thumbs .4s, one
  curve). The overlay itself never animates: it appears and disappears
  at once, because a fading crosshair is a wrong crosshair. The kit's
  `prefers-reduced-motion` block switches transitions off.

## Gaps

- Not yet seen in the real web views (WebView2 on Windows, WebKitGTK on
  Linux). The headless render is Chromium, like WebView2; WebKitGTK may
  differ in slider styling.
- The colour pipette uses the web view's `EyeDropper`. WebView2 has it;
  WebKitGTK does not, so on Linux the Pick button is absent.
