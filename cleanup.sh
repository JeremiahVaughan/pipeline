#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="${HOME}/deploy/${APP}"

list_file="${SCRIPT_DIR}/deploy_file_list.txt"

if [[ ! -f "$list_file" ]]; then
  echo "Missing $list_file in ${SCRIPT_DIR}" >&2
  exit 1
fi

declare -A keep=()
while IFS= read -r line || [[ -n "$line" ]]; do
  line="${line%$'\r'}"
  [[ -z "$line" ]] && continue
  # Compare by basename only.
  line="${line%/}"
  base="${line##*/}"
  [[ -z "$base" ]] && continue
  keep["$base"]=1
done < "$list_file"

echo "Keep list (basenames):"
printf '  %s\n' "${!keep[@]}" | sort

shopt -s dotglob nullglob
for entry in "$SCRIPT_DIR"/*; do
  entry_base="$(basename "$entry")"
  if [[ -z "${keep[$entry_base]+x}" ]]; then
    echo "Deleting (not in keep list): $entry"
    rm -rf -- "$entry"
  fi
done
