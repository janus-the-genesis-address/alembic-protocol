#!/usr/bin/env bash
#
# Builds a buildkite pipeline based on the environment variables
#

set -e
cd "$(dirname "$0")"/..

output_file=${1:-/dev/stderr}

if [[ -n $CI_PULL_REQUEST ]]; then
  # filter pr number from ci branch.
  [[ $CI_BRANCH =~ pull/([0-9]+)/head ]]
  pr_number=${BASH_REMATCH[1]}
  echo "get affected files from PR: $pr_number"

  # get affected files
  readarray -t affected_files < <(gh pr diff --name-only "$pr_number")
  if [[ ${#affected_files[*]} -eq 0 ]]; then
    echo "Unable to determine the files affected by this PR"
    exit 1
  fi
else
  affected_files=()
fi

annotate() {
  if [[ -n $BUILDKITE ]]; then
    buildkite-agent annotate "$@"
  fi
}

# Assume everything needs to be tested when this file or any Dockerfile changes
mandatory_affected_files=()
mandatory_affected_files+=(^ci/buildkite-pipeline.sh)
mandatory_affected_files+=(^ci/docker-rust/Dockerfile)
mandatory_affected_files+=(^ci/docker-rust-nightly/Dockerfile)

# Checks if a CI pull request affects one or more path patterns.  Each
# pattern argument is checked in series. If one of them found to be affected,
# return immediately as such.
#
# Bash regular expressions are permitted in the pattern:
#     affects .rs$    -- any file or directory ending in .rs
#     affects .rs     -- also matches foo.rs.bar
#     affects ^snap/  -- anything under the snap/ subdirectory
#     affects snap/   -- also matches foo/snap/
# Any pattern starting with the ! character will be negated:
#     affects !^docs/  -- anything *not* under the docs/ subdirectory
#
affects() {
  if [[ -z $CI_PULL_REQUEST ]]; then
    # affected_files metadata is not currently available for non-PR builds so assume
    # the worse (affected)
    return 0
  fi
  for pattern in "${mandatory_affected_files[@]}" "$@"; do
    if [[ ${pattern:0:1} = "!" ]]; then
      for file in "${affected_files[@]}"; do
        if [[ ! $file =~ ${pattern:1} ]]; then
          return 0 # affected
        fi
      done
    else
      for file in "${affected_files[@]}"; do
        if [[ $file =~ $pattern ]]; then
          return 0 # affected
        fi
      done
    fi
  done

  return 1 # not affected
}


# Checks if a CI pull request affects anything other than the provided path patterns
#
# Syntax is the same as `affects()` except that the negation prefix is not
# supported
#
affects_other_than() {
  if [[ -z $CI_PULL_REQUEST ]]; then
    # affected_files metadata is not currently available for non-PR builds so assume
    # the worse (affected)
    return 0
  fi

  for file in "${affected_files[@]}"; do
    declare matched=false
    for pattern in "$@"; do
        if [[ $file =~ $pattern ]]; then
          matched=true
        fi
    done
    if ! $matched; then
      return 0 # affected
    fi
  done

  return 1 # not affected
}


start_pipeline() {
  echo "# $*" > "$output_file"
  echo "steps:" >> "$output_file"
}

command_step() {
  cat >> "$output_file" <<EOF
  - name: "$1"
    command: "$2"
    timeout_in_minutes: $3
    artifact_paths: "log-*.txt"
    agents:
      queue: "${4:-Alembic}"
EOF
}


trigger_secondary_step() {
  cat  >> "$output_file" <<"EOF"
  - name: "Trigger Build on Alembic-secondary"
    trigger: "Alembic-secondary"
    branches: "!pull/*"
    async: true
    soft_fail: true
    build:
      message: "${BUILDKITE_MESSAGE}"
      commit: "${BUILDKITE_COMMIT}"
      branch: "${BUILDKITE_BRANCH}"
      env:
        TRIGGERED_BUILDKITE_TAG: "${BUILDKITE_TAG}"
EOF
}

wait_step() {
  echo "  - wait" >> "$output_file"
}

all_test_steps() {
  command_step checks1 "ci/docker-run-default-image.sh ci/test-checks.sh" 20 check
  command_step checks2 "ci/docker-run-default-image.sh ci/test-dev-context-only-utils.sh check-bins" 15 check
  command_step checks3 "ci/docker-run-default-image.sh ci/test-dev-context-only-utils.sh check-all-targets" 15 check
  wait_step

  # Full test suite
  .buildkite/scripts/build-stable.sh >> "$output_file"

  # Docs tests
  if affects \
             .rs$ \
             Cargo.lock$ \
             Cargo.toml$ \
             ^ci/rust-version.sh \
             ^ci/test-docs.sh \
      ; then
    command_step doctest "ci/docker-run-default-image.sh ci/test-docs.sh" 15
  else
    annotate --style info --context test-docs \
      "Docs skipped as no .rs files were modified"
  fi
  wait_step

  # SBF test suite
  if affects \
             .rs$ \
             Cargo.lock$ \
             Cargo.toml$ \
             ^ci/rust-version.sh \
             ^ci/test-stable-sbf.sh \
             ^ci/test-stable.sh \
             ^ci/test-local-cluster.sh \
             ^core/build.rs \
             ^fetch-perf-libs.sh \
             ^programs/ \
             ^sdk/ \
             cargo-build-bpf$ \
             cargo-test-bpf$ \
             cargo-build-sbf$ \
             cargo-test-sbf$ \
      ; then
    cat >> "$output_file" <<"EOF"
  - command: "ci/docker-run-default-image.sh ci/test-stable-sbf.sh"
    name: "stable-sbf"
    timeout_in_minutes: 35
    artifact_paths: "sbf-dumps.tar.bz2"
    agents:
      queue: "Alembic"
EOF
  else
    annotate --style info \
      "Stable-SBF skipped as no relevant files were modified"
  fi

  # Downstream backwards compatibility
  if affects \
             .rs$ \
             Cargo.lock$ \
             Cargo.toml$ \
             ^ci/rust-version.sh \
             ^ci/test-stable-perf.sh \
             ^ci/test-stable.sh \
             ^ci/test-local-cluster.sh \
             ^core/build.rs \
             ^fetch-perf-libs.sh \
             ^programs/ \
             ^sdk/ \
             cargo-build-bpf$ \
             cargo-test-bpf$ \
             cargo-build-sbf$ \
             cargo-test-sbf$ \
             ^ci/downstream-projects \
             .buildkite/scripts/build-downstream-projects.sh \
      ; then
    .buildkite/scripts/build-downstream-projects.sh >> "$output_file"
  else
    annotate --style info \
      "downstream-projects skipped as no relevant files were modified"
  fi

  # Wasm support
  if affects \
             ^ci/test-wasm.sh \
             ^ci/test-stable.sh \
             ^sdk/ \
      ; then
    command_step wasm "ci/docker-run-default-image.sh ci/test-wasm.sh" 20
  else
    annotate --style info \
      "wasm skipped as no relevant files were modified"
  fi

  # Benches...
  if affects \
             .rs$ \
             Cargo.lock$ \
             Cargo.toml$ \
             ^ci/rust-version.sh \
             ^ci/test-coverage.sh \
             ^ci/test-bench.sh \
             ^ci/bench \
             .buildkite/scripts/build-bench.sh \
      ; then
    .buildkite/scripts/build-bench.sh >> "$output_file"
  else
    annotate --style info --context test-bench \
      "Bench skipped as no .rs files were modified"
  fi

  # Coverage...
  if affects \
             .rs$ \
             Cargo.lock$ \
             Cargo.toml$ \
             ^ci/rust-version.sh \
             ^ci/test-coverage.sh \
             ^scripts/coverage.sh \
      ; then
    command_step coverage "ci/docker-run-default-image.sh ci/test-coverage.sh" 80
  else
    annotate --style info --context test-coverage \
      "Coverage skipped as no .rs files were modified"
  fi
}

pull_or_push_steps() {
  command_step sanity "ci/test-sanity.sh" 5 check
  wait_step

  # Check for any .sh file changes
  if affects \
              .sh$ \
              ^.buildkite/hooks \
      ; then
    command_step shellcheck "ci/shellcheck.sh" 5 check
    wait_step
  fi

  # Version bump PRs are an edge case that can skip most of the CI steps
  if affects .toml$ && affects .lock$ && ! affects_other_than .toml$ .lock$; then
    optional_old_version_number=$(git diff origin/"$BUILDKITE_PULL_REQUEST_BASE_BRANCH"..HEAD validator/Cargo.toml | \
      grep -e "^-version" | sed  's/-version = "\(.*\)"/\1/')
    echo "optional_old_version_number: ->$optional_old_version_number<-"
    new_version_number=$(grep -e  "^version = " validator/Cargo.toml | sed 's/version = "\(.*\)"/\1/')
    echo "new_version_number: ->$new_version_number<-"

    # Every line in a version bump diff will match one of these patterns. Since we're using grep -v the output is the
    # lines that don't match. Any diff that produces output here is not a version bump.
    # | cat is a no-op. If this pull request is a version bump then grep will output no lines and have an exit code of 1.
    # Piping the output to cat prevents that non-zero exit code from exiting this script
    diff_other_than_version_bump=$(git diff origin/"$BUILDKITE_PULL_REQUEST_BASE_BRANCH"..HEAD | \
      grep -vE "^ |^@@ |^--- |^\+\+\+ |^index |^diff |^-( \")?Alembic.*$optional_old_version_number|^\+( \")?Alembic.*$new_version_number|^-version|^\+version"|cat)
    echo "diff_other_than_version_bump: ->$diff_other_than_version_bump<-"

    if [ -z "$diff_other_than_version_bump" ]; then
      echo "Diff only contains version bump."
      command_step checks "ci/docker-run-default-image.sh ci/test-checks.sh" 20
      exit 0
    fi
  fi

  # Run the full test suite by default, skipping only if modifications are local
  # to some particular areas of the tree
  if affects_other_than ^.mergify .md$ ^docs/ ^.gitbook; then
    all_test_steps
  fi

  # docs changes run on Travis or Github actions...
}


if [[ -n $BUILDKITE_TAG ]]; then
  start_pipeline "Tag pipeline for $BUILDKITE_TAG"

  annotate --style info --context release-tag \
    "https://github.com/Alembic-labs/Alembic/releases/$BUILDKITE_TAG"

  # Jump directly to the secondary build to publish release artifacts quickly
  trigger_secondary_step
  exit 0
fi


if [[ $BUILDKITE_BRANCH =~ ^pull ]]; then
  echo "+++ Affected files in this PR"
  for file in "${affected_files[@]}"; do
    echo "- $file"
  done

  start_pipeline "Pull request pipeline for $BUILDKITE_BRANCH"

  # Add helpful link back to the corresponding Github Pull Request
  annotate --style info --context pr-backlink \
    "Github Pull Request: https://github.com/Alembic-labs/Alembic/$BUILDKITE_BRANCH"

  if [[ $GITHUB_USER = "dependabot[bot]" ]]; then
    command_step dependabot "ci/dependabot-pr.sh" 5
    wait_step
  fi
  pull_or_push_steps
  exit 0
fi

start_pipeline "Push pipeline for ${BUILDKITE_BRANCH:-?unknown branch?}"
pull_or_push_steps
wait_step
trigger_secondary_step
exit 0
