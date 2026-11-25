#!/bin/sh
Xvfb :1 -screen 0 1600x900x24 >/tmp/xvfb.log 2>&1 &
sleep 2
export DISPLAY=:1
eval "$(dbus-launch --sh-syntax)"
env | grep DBUS_SESSION_BUS_ADDRESS > /tmp/bus.env
xfwm4 >/tmp/xfwm.log 2>&1 &
xfce4-panel >/tmp/panel.log 2>&1 &
xfdesktop >/tmp/desk.log 2>&1 &
sleep infinity
