#!/usr/bin/env bash
set -x
set -eo pipefail

if ! [ -x "$(command -v python3)" ]; then
  echo >&2 "Error: pyhton3 is not installed"
  exit 1
fi

python3 -m smtpd -n -c DebuggingServer 127.0.0.1:2525
