#!/usr/bin/env bash
####################################
#
#	GESCHREVEN DOOR AI!!!
#
####################################
set -euo pipefail

usage() {
  cat <<EOF
Usage: $(basename "$0") [--all | --dirs dir1,dir2,...] [--set KEY=VALUE ...] [--unset KEY ...]

Options:
  --all                   Operate on all subdirectories in the current folder.
  --dirs DIR1,DIR2,...    Comma-separated list of directories to target.
  --set KEY=VALUE         Add or update this key/value. Can be repeated.
  --unset KEY             Remove this key. Can be repeated.
  -h, --help              Show this help message.

Examples:
  # Add API_KEY to all folders
  ./env-manager.sh --all --set API_KEY=abcdef

  # Remove DEBUG from folder1 and folder3
  ./env-manager.sh --dirs folder1,folder3 --unset DEBUG

  # Update two values in all folders
  ./env-manager.sh --all --set HOST=example.com --set PORT=8080

  # Mixed add/remove in specific folders
  ./env-manager.sh --dirs foo,bar --set MODE=prod --unset DEBUG
EOF
  exit 1
}

# parse args
TARGET_ALL=false
TARGET_DIRS=()
declare -a TO_SET=()
declare -a TO_UNSET=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    --all)
      TARGET_ALL=true
      shift
      ;;
    --dirs)
      IFS=',' read -r -a TARGET_DIRS <<< "$2"
      shift 2
      ;;
    --set)
      TO_SET+=("$2")
      shift 2
      ;;
    --unset)
      TO_UNSET+=("$2")
      shift 2
      ;;
    -h|--help)
      usage
      ;;
    *)
      echo "Unknown argument: $1" >&2
      usage
      ;;
  esac
done

if ! $TARGET_ALL && [[ ${#TARGET_DIRS[@]} -eq 0 ]]; then
  echo "Error: must supply either --all or --dirs" >&2
  usage
fi

# Build list of folders
if $TARGET_ALL; then
  # all subdirectories (that contain a .env or not)
  mapfile -t FOLDERS < <(find . -maxdepth 1 -type d ! -name . | sed 's|^\./||')
else
  FOLDERS=("${TARGET_DIRS[@]}")
fi

# Main loop
for dir in "${FOLDERS[@]}"; do
  envfile="$dir/.env"
  # create if missing
  if [[ ! -f $envfile ]]; then
    echo "Creating $envfile"
    mkdir -p "$dir"
    touch "$envfile"
  fi

  # Remove keys first
  for key in "${TO_UNSET[@]}"; do
    # delete any line starting with KEY=
    if grep -qE "^${key}=" "$envfile"; then
      echo "[$dir] Removing '$key'"
      sed -i.bak "/^${key}=/d" "$envfile" && rm -f "$envfile".bak
    fi
  done

  # Add/update keys
  for pair in "${TO_SET[@]}"; do
    key="${pair%%=*}"
    value="${pair#*=}"
    if grep -qE "^${key}=" "$envfile"; then
      echo "[$dir] Updating '$key' to '$value'"
      # replace the line
      sed -i.bak "s~^${key}=.*~${key}=${value}~" "$envfile" && rm -f "$envfile".bak
    else
      echo "[$dir] Adding '$key=${value}'"
      echo "${key}=${value}" >> "$envfile"
    fi
  done
done

echo "Done."
