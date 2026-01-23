#!/bin/bash
set -e

export APP=pipeline
get_files() {
    local files=(
        "./static"
        "./crates/db/migrations"
        "./config/${ENVIRONMENT}/config.json"
        "/tmp/${APP}"
        "./${APP}.service"
        "./remote-deploy.sh"
    )
    printf "%s\n" "${files[@]}"
}

source "./deploy_base.sh" "$@"
