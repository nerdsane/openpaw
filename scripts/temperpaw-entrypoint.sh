#!/usr/bin/env sh
set -eu

export DD_SERVICE="${DD_SERVICE:-openpaw}"
export DD_ENV="${DD_ENV:-prod}"
export DD_VERSION="${DD_VERSION:-${BUILD_SHA:-unknown}}"

if [ "${DD_PROFILING_ENABLED:-false}" = "true" ]; then
  if command -v ddprof >/dev/null 2>&1; then
    exec ddprof \
      --service "${DD_SERVICE}" \
      --environment "${DD_ENV}" \
      --service_version "${DD_VERSION}" \
      ./temperpaw
  fi
  echo "DD_PROFILING_ENABLED=true but ddprof is not installed; starting without native profiling" >&2
fi

exec ./temperpaw
