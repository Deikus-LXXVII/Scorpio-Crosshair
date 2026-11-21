# Languages

Owner, 2026-11-21: a language choice in the program, Russian first, then the
popular languages. The 0.1.3 installer sent to Microsoft for SmartScreen goes
out with the translations.

## Shape

- English is written in the code. Every other language is a dictionary from
  the English text to its translation: `src-tauri/locales/<code>.json`. A
  value is a string, or for counts an object of plural forms keyed by
  `Intl.PluralRules` category (`one`, `few`, `many`, `other`).
- The dictionaries are built into the binary (`i18n.rs`, `include_str!`),
  outside the web view's files, so they ship once. Nothing is kept parsed: the
  settings window gets the chosen dictionary in `view`, and the few texts made
  in Rust (tray menu, error messages) read the built-in JSON when asked.
- Setting `language`: `"auto"` (default) or a code. Auto takes Windows'
  display language (`GetUserDefaultUILanguage`) or on Linux `LANGUAGE`,
  `LC_ALL`, `LC_MESSAGES`, `LANG`, and falls back to English.
- Changing the language saves and reloads the settings window; the tray
  relabels itself.
- The problem report stays in English: it is written for the developer.
- Fonts: Latin, Latin Extended (Polish, Turkish) and Cyrillic subsets of
  Forum and Inter ship with the app; Chinese, Japanese and Korean use the
  system's fonts.

## Languages

English, Russian, Ukrainian, German, French, Spanish, Portuguese (Brazil),
Polish, Turkish, Simplified Chinese, Japanese, Korean.

## Checks

- `i18n::tests::every_language_translates_every_text_and_nothing_else`: every
  `t( '…' )`, `tn( n, '…', '…' )`, `data-i18n`, `tr("…")` and crosshair style
  name is in every dictionary, nothing else is, and each translation keeps the
  same `{marks}`.
- Screens of every tab in Russian and German (long words) for overflow.

## Done with it

- Help lines that only explained a control were removed from the settings
  window (owner, 2026-11-21). Kept: the Hide from screen capture warning, the
  launcher games count, what Fullscreen windows means, empty states, the
  current position, and "Escape cancels, Backspace clears." over the shortcuts.
- "CS2 style" beside "Your image" is now "Classic".

## Gaps

- Translations other than Russian were written without a native speaker's
  check.
