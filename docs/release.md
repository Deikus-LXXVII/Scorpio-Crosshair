# Releasing Scorpio

Steps for a release, in order. A release needs the owner's word.

1. Bump the version in `package.json`, `src-tauri/Cargo.toml`, the `scorpio`
   entry of `src-tauri/Cargo.lock`, `src-tauri/tauri.conf.json`, and the
   version and date line of `docs/site/llms.txt`. Run `node tools/site/build.js`.
2. Linux on the VPS: clippy `-D warnings`, `cargo test`, then
   `npx tauri build --bundles appimage,deb` with `createUpdaterArtifacts` off.
3. Windows on the gaming PC: `npx tauri build --bundles nsis`, same config.
4. Sign the setup `.exe` and the AppImage with `npx tauri signer sign -f
   ~/.tauri/scorpio.key`, write `latest.json` (never change its endpoint or
   the updater key) and `SHA256SUMS.txt`.
5. Merge into `main`, push, tag `vX.Y.Z`, `gh release create` with the notes,
   the five files, `latest.json` and `SHA256SUMS.txt`, marked latest.
6. Deploy the site: `SCORPIO_SITE_HOST=usa-vps tools/site/deploy.sh`.
7. AUR: `tools/aur.sh X.Y.Z` (needs docker), commit `packaging/aur`. Pushing
   it to the AUR is done from the owner's AUR account.
8. Tell the T3 session that keeps lxxvll.com, in one line: the version, the
   date, and whether "Not tested yet" in `llms.txt` changed. Its pages repeat
   those facts.
9. winget: once `LXXVII.Scorpio` is in winget-pkgs, submit the new version's
   manifest.
