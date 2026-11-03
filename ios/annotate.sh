#!/bin/bash
# Turn compiler errors in a log into GitHub annotations (visible on the run
# page and through the API, without downloading logs).
# usage: annotate.sh <logfile>
grep -E '(error|warning: unhandled|fatal error):' "$1" | grep -E '^/|error:' | sort -u | head -30 | while IFS= read -r line; do
  if [[ "$line" =~ ^(/[^:]+):([0-9]+):([0-9]+):\ (error|fatal\ error):\ (.*)$ ]]; then
    file="${BASH_REMATCH[1]#"$GITHUB_WORKSPACE/"}"
    echo "::error file=${file},line=${BASH_REMATCH[2]},col=${BASH_REMATCH[3]}::${BASH_REMATCH[5]}"
  else
    echo "::error::${line}"
  fi
done
