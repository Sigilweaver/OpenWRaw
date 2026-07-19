#!/usr/bin/env bash
# Refuse to say a commit is release-ready unless the most recent ci.yml and
# audit.yml runs for that commit both completed successfully.
#
# publish.yml triggers directly on `push: tags: ["v*"]` and GitHub Actions
# has no way for one workflow file to `needs:` a job defined in a separate
# workflow file, so this check has to run before the tag is created (see
# RELEASING.md and https://github.com/Sigilweaver/OpenWRaw/issues/14).
#
# Usage: scripts/check-release-ready.sh [ref]
#   ref defaults to HEAD.

set -euo pipefail

REF="${1:-HEAD}"
# `^{commit}` peels annotated tags to the commit they point at; git rev-parse
# on an annotated tag alone returns the tag object's own SHA, which never
# matches a workflow run's head SHA.
SHA="$(git rev-parse "${REF}^{commit}")"

check_workflow() {
  local workflow="$1"
  local runs
  runs="$(gh run list -w "$workflow" -c "$SHA" --json status,conclusion,url -L 1)"

  if [[ "$(echo "$runs" | jq 'length')" -eq 0 ]]; then
    echo "FAIL: no run of $workflow found for commit $SHA" >&2
    return 1
  fi

  local status conclusion url
  status="$(echo "$runs" | jq -r '.[0].status')"
  conclusion="$(echo "$runs" | jq -r '.[0].conclusion')"
  url="$(echo "$runs" | jq -r '.[0].url')"

  if [[ "$status" != "completed" ]]; then
    echo "FAIL: latest $workflow run for $SHA has not completed (status=$status) - $url" >&2
    return 1
  fi

  if [[ "$conclusion" != "success" ]]; then
    echo "FAIL: latest $workflow run for $SHA did not succeed (conclusion=$conclusion) - $url" >&2
    return 1
  fi

  echo "OK: $workflow succeeded for $SHA - $url"
  return 0
}

ready=1
check_workflow "ci.yml" || ready=0
check_workflow "audit.yml" || ready=0

if [[ "$ready" -eq 1 ]]; then
  echo "Release ready: ci.yml and audit.yml are both green for $SHA"
  exit 0
fi

exit 1
