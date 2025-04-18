####################################
#
#	GESCHREVEN DOOR AI!!!
#
####################################
# save this as kuma-find.sh and then run:
#   source kuma-find.sh "some name"

#!/usr/bin/env bash
BASE_DIR="$HOME/Services/Webcom"
# ─── ensure we're being sourced so that `cd` persists ─────────────────────────
# if [ "$0" = "${BASH_SOURCE[0]}" ]; then
#   echo "⚠️  Please source this script so the 'cd' sticks:"
#   echo "    source $0 \"Name to find\""
#   exit 1
# fi

# ─── colors ───────────────────────────────────────────────────────────────────
GREEN='\033[0;32m'; YELLOW='\033[1;33m'; RED='\033[0;31m'; NC='\033[0m'

# ─── usage ────────────────────────────────────────────────────────────────────
if [ -z "$1" ]; then
  echo -e "${RED}Usage:${NC} source $0 \"Name to find\""
  return 1
fi

input="$1"
lc_input="${input,,}"            # lowercase
search_name="$(tr '[:lower:]' '[:upper:]' <<< "${lc_input:0:1}")${lc_input:1}"  # Title case

# ─── collect all kuma/name files ──────────────────────────────────────────────
declare -A folder_name_map
while IFS= read -r -d '' file; do
  folder="$(dirname "$(dirname "$file")")"
  name="$(<"$file")"
  folder_name_map["$folder"]="$name"
done < <(find "$BASE_DIR" -maxdepth 3 -type f -path '*/kuma/name' -print0)

# ─── find exact (case‐insensitive) matches ────────────────────────────────────
matches=()
for d in "${!folder_name_map[@]}"; do
  if [ "${folder_name_map[$d],,}" = "$lc_input" ]; then
    matches+=("$d")
  fi
done

count=${#matches[@]}

# ─── if exactly one match ─────────────────────────────────────────────────────
if [ "$count" -eq 1 ]; then
  folder="${matches[0]}"
  echo -e "${GREEN}Found:${NC} $folder"
  cd "$folder" || return
  return
fi

# ─── special 2‐match with one starting "_" ────────────────────────────────────
if [ "$count" -eq 2 ]; then
  us=() non=()
  for d in "${matches[@]}"; do
    [[ $(basename "$d") == _* ]] && us+=("$d") || non+=("$d")
  done
  if [ "${#us[@]}" -eq 1 ] && [ "${#non[@]}" -eq 1 ]; then
    echo -e "${YELLOW}Ignored (leading underscore):${NC} ${us[0]}"
    echo -e "${GREEN}Auto‐selected:${NC} ${non[0]}"
    cd "${non[0]}" || return
    return
  fi
fi

# ─── more than one match → prompt the user ────────────────────────────────────
if [ "$count" -gt 1 ]; then
  echo "Multiple matches for “$input”:"
  for i in "${!matches[@]}"; do
    printf "  %2d) %s\n" "$((i+1))" "${matches[i]}"
  done
  printf "Choose [1-%d]: " "$count"
  read -r choice
  if ! [[ "$choice" =~ ^[1-9][0-9]*$ ]] || [ "$choice" -lt 1 ] || [ "$choice" -gt "$count" ]; then
    echo -e "${RED}Invalid choice${NC} – aborting."
    return 1
  fi
  sel="${matches[$((choice-1))]}"
  echo -e "${GREEN}You picked:${NC} $sel"
  cd "$sel" || return
  return
fi

# ─── no exact matches → find closest by Levenshtein ──────────────────────────
# a simple Levenshtein‐distance function in pure bash
levenshtein() {
  local s1=$1 s2=$2
  local len1=${#s1} len2=${#s2}
  local matrix; matrix=()
  for ((i=0; i<=len1; i++)); do matrix[i*(len2+1)]=${i}; done
  for ((j=0; j<=len2; j++)); do matrix[j]=${j}; done
  for ((i=1; i<=len1; i++)); do
    for ((j=1; j<=len2; j++)); do
      [[ "${s1:i-1:1}" == "${s2:j-1:1}" ]] && cost=0 || cost=1
      del=$(( matrix[(i-1)*(len2+1)+j] + 1 ))
      ins=$(( matrix[i*(len2+1)+j-1] + 1 ))
      sub=$(( matrix[(i-1)*(len2+1)+j-1] + cost ))
      # pick min of del,ins,sub
      if (( del < ins && del < sub )); then
        matrix[i*(len2+1)+j]=$del
      elif (( ins < sub )); then
        matrix[i*(len2+1)+j]=$ins
      else
        matrix[i*(len2+1)+j]=$sub
      fi
    done
  done
  echo "${matrix[len1*(len2+1)+len2]}"
}

best_dist=9999; best_folder=; best_name=
for d in "${!folder_name_map[@]}"; do
  name="${folder_name_map[$d]}"
  # compare lowercase versions
  dist=$(levenshtein "${name,,}" "$lc_input")
  if (( dist < best_dist )); then
    best_dist=$dist
    best_folder=$d
    best_name=$name
  fi
done

if [ -z "$best_folder" ]; then
  echo -e "${RED}No kuma/name files found in any subfolder.${NC}"
  return 1
else
  echo -e "${RED}No exact match for “$input”.${NC}"
  echo -e "Closest is ${GREEN}$best_folder${NC} (name: ${best_name})."
  return 1
fi
