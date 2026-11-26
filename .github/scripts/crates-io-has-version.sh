#!/usr/bin/env bash
# Usage: crates-io-has-version.sh <crate> <version>
#
# Exits 0 if <crate>@<version> is in the crates.io index, 1 if it is not (including when the
# crate has never been published), and 2 if the index could not be read. Reads the sparse
# index, which is what cargo itself resolves against, rather than the web API.
set -euo pipefail

# Checked by hand: `set -u` would exit 1 on a missing argument, which reads as "not published".
if [ "$#" -ne 2 ]; then
  echo "usage: crates-io-has-version.sh <crate> <version>" >&2
  exit 2
fi
crate=$1
version=$2

# Sparse-index layout for names of four or more characters: <first two>/<next two>/<name>.
if [ "${#crate}" -lt 4 ]; then
  echo "crates-io-has-version.sh: '$crate' is shorter than 4 characters; index path not handled" >&2
  exit 2
fi
url="https://index.crates.io/${crate:0:2}/${crate:2:2}/${crate}"

body=$(mktemp)
trap 'rm -f "$body"' EXIT

code=$(curl --silent --show-error --retry 3 --retry-all-errors --output "$body" \
  --write-out '%{http_code}' "$url") || exit 2

case "$code" in
  200)
    # One JSON object per published version.
    if jq --exit-status --arg v "$version" 'select(.vers == $v)' "$body" >/dev/null; then
      exit 0
    fi
    exit 1
    ;;
  404) exit 1 ;;
  *)
    echo "crates-io-has-version.sh: $url answered HTTP $code" >&2
    exit 2
    ;;
esac
