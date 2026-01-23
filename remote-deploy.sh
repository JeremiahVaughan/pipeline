#!/bin/bash
set -e
cp "${HOME}/deploy/${APP}/${APP}.service" "${HOME}/.local/share/systemd/user/${APP}.service"
systemctl --user daemon-reload
systemctl --user enable "${APP}.service"
systemctl --user restart "${APP}.service" || systemctl --user start "${APP}.service"
