####################################
#
#	GESCHREVEN DOOR AI!!!
#
####################################
#!/bin/bash

# Get the current working directory
current_dir=$(pwd)

# Iterate over each child folder in the current directory
for folder in */; do
  # Check if the folder name starts with an underscore
  if [[ $folder == _* ]]; then
    continue
  fi

  # Check if the .env file exists in the folder
  if [ -f "$folder/.env" ]; then
    # Read the .env file and extract the MAIL_TO variable
    mail_to=$(grep '^MAIL_TO=' "$folder/.env" | cut -d '=' -f 2-)
    if [ -n "$mail_to" ]; then
      echo "$mail_to"
    fi
  fi
done
