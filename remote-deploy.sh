#!/bin/bash
set -e
cp "${HOME}/deploy/${APP}/${APP}.service" "${HOME}/.local/share/systemd/user/${APP}.service"
systemctl --user daemon-reload
systemctl --user enable "${APP}.service"
mv "${HOME}/deploy/${APP}/app" "${HOME}/deploy/${APP}/app.new"
echo "app.new" >> "${HOME}/deploy/${APP}/deploy_file_list.txt"
systemctl --user restart "${APP}.service" || systemctl --user start "${APP}.service"
${HOME}/deploy/${APP}/cleanup.sh
