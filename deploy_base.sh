#!/bin/bash
set -e

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

DECRYPT_DIR=""
RESOLVED_FILES=()

cleanup() {
    if [[ -n "$DECRYPT_DIR" && -d "$DECRYPT_DIR" ]]; then
        rm -rf "$DECRYPT_DIR"
    fi
}
trap cleanup EXIT

prepare_user_files() {
    RESOLVED_FILES=()
    if [[ ${#USER_FILES[@]} -eq 0 ]]; then
        return
    fi

    for file_path in "${USER_FILES[@]}"; do
        if [[ ! -e "$file_path" ]]; then
            echo "error: file or directory '$file_path' from get_files does not exist" >&2
            exit 1
        fi

        if [[ "$file_path" == *.toml && -f "$file_path" ]]; then
            if [[ -z "$DECRYPT_DIR" ]]; then
                DECRYPT_DIR=$(mktemp -d "/tmp/${APP}_configs.XXXXXX")
            fi

            local base_name
            base_name=$(basename "$file_path")
            local decrypted_path="${DECRYPT_DIR}/${base_name}"
            sops -d "$file_path" > "$decrypted_path"
            RESOLVED_FILES+=("$decrypted_path")
        else
            RESOLVED_FILES+=("$file_path")
        fi
    done
}

USER_FILES=()
if declare -f get_files >/dev/null 2>&1; then
    USER_FILES=($(get_files))
fi


build() {
    local arch="$1"
    
    if declare -F custom_build >/dev/null 2>&1; then
        echo "Using custom build for ${APP}"
        custom_build "$arch"
    else
        echo "Using default build for ${APP}"
        GOOS=linux GOARCH="$arch" cargo build --target-dir "/tmp/${APP}"
    fi
}

deploy_remote() {
    local environment="$1"
    local arch="$2"

    echo "remote deploy triggered for ${environment}"
    build "$arch"
    prepare_user_files
    rsync -avzh --delete -e ssh "${RESOLVED_FILES[@]}" "${environment}:${HOME}/deploy/${APP}"
    ssh "${environment}" "APP=$APP ${HOME}/deploy/${APP}/remote-deploy.sh"
}

deploy_local() {
    local environment="$1"
    local arch="$2"

    echo "local deploy triggered"
    build "$arch"
    prepare_user_files
    rsync -avzh --delete "${RESOLVED_FILES[@]}" "${HOME}/deploy/${APP}"
    APP="$APP" "${HOME}/deploy/${APP}/remote-deploy.sh"
}

if [[ "$ENVIRONMENT" == "development" ]]; then
    deploy_local "$ENVIRONMENT" "$BUILD_ARCH"
else
    deploy_remote "$ENVIRONMENT" "$BUILD_ARCH"
fi
