#!/bin/bash

####################################
#
#	(GROTENDEELS) GESCHREVEN DOOR AI!!!
#
####################################

MAIN_BRANCH="webcom_ical"
DEV_BRANCH="webcom_ical_dev"
DEV_USER="25348"

# Colors for output
GREEN="\033[0;32m"
YELLOW="\033[0;33m"
BLUE="\033[0;34m"
RED="\033[0;31m"
RESET="\033[0m"

# Function to display a progress indicator
show_progress() {
    local pid=$1
    local spinner=('⣾' '⣽' '⣻' '⢿' '⡿' '⣟' '⣯' '⣷')
    local delay=0.1
    local spin_index=0

    # Display the spinner while the command is running
    while kill -0 "$pid" 2>/dev/null; do
        printf "\r${BLUE}[%s]${RESET} ${YELLOW}%s${RESET}" "${spinner[spin_index]}" "$2"
        spin_index=$(( (spin_index + 1) % 8 ))
        sleep $delay
    done

    # Display a success checkmark after the command completes
    printf "\r${GREEN}[✔]${RESET} ${YELLOW}%s${RESET}\n" "$2"
}

build_webcom() {
  cd repo/$MAIN_BRANCH
  (git pull > /dev/null 2>&1) &
  show_progress $! "Pulling Webcom Ical"
  (docker build -t $MAIN_BRANCH . 2>&1) &
  show_progress $! "Building Webcom Ical"
  cd ../../
  echo $(pwd)
}

get_all_directories(){
    # Iterate over each directory in the current directory
    current_dir=$(pwd)
    for dir in "$current_dir"/*; do
        # Check if it is a directory and does not start with "_" or is named "repo"
        if [ -d "$dir" ] && [[ "$(basename "$dir")" != "repo" ]] && [[ "$(basename "$dir")" != _* ]]; then
            restart_directory $dir
        fi
    done
}

restart_directory() {
    current_dir=$(pwd)
    dir_name=$(basename "$1")
    echo -e "${BLUE}Processing directory:${RESET} ${GREEN}$dir_name${RESET}"
    cd "$1" || { echo -e "${RED}Failed to enter directory${RESET}: $1"; continue; }

    # Run Docker Compose commands in the background
    if [ -f "docker-compose.yml" ]; then
        (docker compose up -d >/dev/null 2>&1) &
        show_progress $! "Restarting Docker in $dir_name"
    else
        echo -e "${YELLOW}No docker-compose.yml found${RESET} in ${GREEN}$dir_name${RESET}, skipping..."
    fi

    # Return to the original directory
    cd "$current_dir" || exit
}

while getopts 'cd' OPTION; do
  case "$OPTION" in
    d)
      MAIN_BRANCH=$DEV_BRANCH
      build_webcom
      restart_directory "$(pwd)/$DEV_USER"
      ;;
    c)
      build_webcom
      get_all_directories
      ;;
    ?)
      echo "script usage: $(basename \$0) [-c] <- Compile" >&2
      exit 1
      ;;
  esac
done
if [ $OPTIND -eq 1 ]; then 
    get_all_directories
fi

shift "$(($OPTIND -1))"


echo -e "${GREEN}Script execution completed.${RESET}"
