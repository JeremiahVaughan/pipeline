#!/bin/bash
set -e

export APP=pipeline
list_files() {
    TARGET_DIR="release"
    if [[ "$ENVIRONMENT" == "development" ]]; then
        TARGET_DIR="debug"
    fi

cat <<EOF >> ./target/deploy_file_list.txt
./crates/db/migrations
./target/${TARGET_DIR}/app
./${APP}.service
./remote-deploy.sh
./target/deploy_file_list.txt
./cleanup.sh
EOF

}

source "./deploy_base.sh" "$@"
