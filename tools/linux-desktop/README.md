# Scorpio on an Xfce desktop

A disposable Linux desktop for checking the released AppImage end to end. It
runs in a rootless podman container under a virtual display (Xvfb), with
Xfce's window manager, panel and tray. `~/Games/THE FINALS/Discovery` is a
copy of xterm, so Scorpio counts its window as a game installed through
Lutris.

Build and start, as an unprivileged user with podman:

    cp Scorpio_X.Y.Z_amd64.AppImage tools/linux-desktop/Scorpio.AppImage
    podman build -t scorpio-desktop tools/linux-desktop
    podman run -d --name desk --memory 2g -v "$PWD/tools/linux-desktop:/shots:ro" scorpio-desktop sh /shots/start.sh

Drive it with `podman exec -e DISPLAY=:1 desk sh -c '. /tmp/bus.env; export DBUS_SESSION_BUS_ADDRESS; …'`:
`./squashfs-root/AppRun &` starts Scorpio; `xdotool` focuses windows and
clicks the tray (bottom right); `import -window root /tmp/x.png` and
`podman cp` take a screenshot. Scorpio's log is
`~/.local/share/com.lxxvii.scorpio/logs/scorpio.log`.

Run on 2026-11-25 with 0.1.6; the results are in README's test table and
`docs/budget.md`. Remove it afterwards with `podman rm -f desk` and
`podman rmi scorpio-desktop`.
