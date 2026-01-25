#!/bin/bash
set -euo pipefail

APP="${APP:?Must set APP}"
ENVIRONMENT="${1:-development}"

case "$ENVIRONMENT" in
    production|staging)
        BUILD_ARCH=arm64
        ;;
    development)
        BUILD_ARCH=amd64
        ;;
    *)
        echo "Usage: $0 [production|staging|development]"
        exit 1
        ;;
esac

RESOLVED_FILES=()

cleanup() {
    rm ./target/config.toml
}
trap cleanup EXIT

prepare_user_files() {
    local decrypted_path="./target/config.toml"
    sops -d "./config/${ENVIRONMENT}/config.toml" > "$decrypted_path"

cat <<EOF > ./target/deploy_file_list.txt
${decrypted_path}
EOF

}

build() {
    local arch="$1"
    
    if declare -F custom_build >/dev/null 2>&1; then
        echo "Using custom build for ${APP}"
        custom_build "$arch"
    else
        echo "Using default build for ${APP}"
        GOOS=linux GOARCH="$arch" cargo build
    fi
}

deploy_remote() {
    local environment="$1"
    local arch="$2"

    echo "remote deploy triggered for ${environment}"
    build "$arch"
    rsync -arvzh --no-relative \
        --delete --delete-missing-args \
        --files-from="./target/deploy_file_list.txt" \
        --filter='protect app.new' \
        -e ssh \
        ./ "${environment}:${HOME}/deploy/${APP}/"
    ssh "${environment}" "APP=$APP ${HOME}/deploy/${APP}/remote-deploy.sh"
}

deploy_local() {
    # protecting the deployed binary since it remains running until we shutdown the server. Doing this keeps the server from crashing and reduces the apps downtime because we don't turn off the service until all the data is present on the target machine.
    local environment="$1"
    local arch="$2"

    echo "remote deploy triggered for ${environment}"
    rsync -arvzh --no-relative \
        --delete --delete-missing-args \
        --files-from="./target/deploy_file_list.txt" \
        --filter='protect app.new' \
        ./ "${HOME}/deploy/${APP}/"
    APP="$APP" "${HOME}/deploy/${APP}/remote-deploy.sh"
}

prepare_user_files
list_files
if [[ "$ENVIRONMENT" == "development" ]]; then
    deploy_local "$ENVIRONMENT" "$BUILD_ARCH"
else
    deploy_remote "$ENVIRONMENT" "$BUILD_ARCH"
fi
