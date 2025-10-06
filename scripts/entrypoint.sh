#!/bin/bash
set -e

# Start geckodriver in background
geckodriver --binary=/opt/firefox/firefox --log=fatal --host=127.0.0.1 --port=4444 >/dev/null 2>&1 &

cd /usr/src/webcom_ical
# Start your Rust app (will connect to localhost:4444)
webcom_ical