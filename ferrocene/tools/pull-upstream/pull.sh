#!/bin/bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: The Ferrocene Developers

set -euo pipefail
IFS=$'\n\t'

UPSTREAM_REPO="https://github.com/rust-lang/rust"
TEMP_BRANCH="pull-upstream-temp--do-not-use-for-real-code"
DIRECTORIES_CONTAINING_LOCKFILES=("" "src/bootstrap/")
GENERATED_COMPLETIONS_DIR="src/etc/completions/"

# Set a default max of merges per PR to 30, if it was not overridden in the
# environment.
if [[ -z "${MAX_MERGES_PER_PR+x}" ]]; then
    MAX_MERGES_PER_PR=30
fi

# Print all files with the `ferrocene-avoid-pulling-from-upstream` attribute.
#
# `sort | uniq` is used because during merges files might show up multiple
# times if they have a conflict, and we don't want that.
excluded_files() {
    git ls-files \
        | git check-attr ferrocene-avoid-pulling-from-upstream --stdin \
        | grep ' set$' \
        | sed 's/:.*$//' \
        | sort \
        | uniq
}

if [[ $# -lt 1 ]] || [[ $# -gt 3 ]]; then
    echo "usage: $0 <upstream-branch> [base-branch] [upstream-commit]"
    exit 1
fi
upstream_branch="$1"
if [[ $# -ge 2 ]]; then
    current_branch="$2"
else
    current_branch="$(git branch --show-current)"
fi
if [[ $# -ge 3 ]]; then
    upstream_commit="$3"
else
    upstream_commit="FETCH_HEAD"  # Latest commit in the branch we pull.
fi

# Move to the root of the repository to avoid the script from misbehaving.
cd "$(git rev-parse --show-toplevel)"

# Safety check to avoid messing with uncommitted changes.
# Submodules are updated before that, as submodules needing an update should
# not block merging chnages from upstream.
git submodule update --init
if ! git diff-index --quiet HEAD; then
    echo "pull-upstream: the current branch contains uncommitted changes!"
    echo "pull-upstream: make sure all changes are committed before running this script."
    exit 1
fi

# Make sure the temporary branch doesn't exist yet.
if git rev-parse --quiet --verify "${TEMP_BRANCH}" > /dev/null; then
    git branch -D "${TEMP_BRANCH}"
fi

git fetch "${UPSTREAM_REPO}" "${upstream_branch}"

# Avoid creating extra-large PRs by limiting the amount of merge commits
# included in automated pulls to ${MAX_MERGES_PER_DAY}.
partial_pull=no
if [[ "${upstream_commit}" = "FETCH_HEAD" ]]; then
    fetch_head="$(git rev-parse FETCH_HEAD)"
    upstream_commit="$(git log HEAD..FETCH_HEAD --first-parent --format="%H" | tail -n "${MAX_MERGES_PER_PR}" | head -n 1)"
    if [[ "${upstream_commit}" = "" ]]; then
        # When the branch is up to date, the `git log` above rightfully doesn't
        # return any difference between the two refs, resulting in an empty
        # ${upstream_commit}. To prevent the rest of the script from misbehaving,
        # we revert the commit back to FETCH_HEAD.
        upstream_commit="FETCH_HEAD"
    elif [[ "${upstream_commit}" != "${fetch_head}" ]]; then
        echo "pull-upstream: pulling at most ${MAX_MERGES_PER_PR}, even though more commits are available"
        partial_pull=yes
    fi
fi

git checkout -b "${TEMP_BRANCH}" "${upstream_commit}"

# Delete all the files excluded from the pull. Those files are marked with the
# `ferrocene-avoid-pulling-from-upstream` in `.gitattributes`.
git checkout "${current_branch}" -- .gitattributes
excluded_files | xargs git rm
git checkout FETCH_HEAD -- .gitattributes

git commit -F- <<EOF
remove excluded files from upstream

This commit is generated by \`ferrocene/tools/pull-upstream/pull.sh\`.
The list of excluded files is defined in \`.gitattributes\`.
EOF

# Note that the generate_pr_body.py script relies on this commit message to
# format its output. When changing this, make sure you are not breaking it.
if [[ "${partial_pull}" = "yes" ]]; then
    merge_message="pull new changes from upstream (partial)"
else
    merge_message="pull new changes from upstream"
fi

git checkout "${current_branch}"
if ! git merge "${TEMP_BRANCH}" --no-edit -m "${merge_message}"; then
    # Merging failed, but the script might be able to resolve all the conflicts
    # on its own. This tries to resolve known conflicts and finish the merge.
    # If not all conflicts were resolved, control is given back to the user.

    # Files excluded by the pull that are also present in Ferrocene (for example
    # a different README) will cause merge conflicts. In those cases we always
    # want to preserve Ferrocene's version, so we can resolve the conflict
    # automatically.
    for file in $(excluded_files); do
        echo "pull-upstream: automatically resolving conflict for ${file}..."
        git show "${current_branch}:${file}" > "${file}"
        git add "${file}"
        echo "pull-upstream: automatically resolved conflict for ${file}"
    done

    # Git attempts to merge submodule bumps correctly, but it only works if one
    # of the two branches has the same submodule commit as the merge base. If
    # that's not true (for example if we get behind with pulls), git refuses to
    # merge automatically and outputs this fairly confusing diff:
    #
    # - Subproject commit 03bc66b55c290324bd46eb22e369c8fae1908f91
    #  -Subproject commit 694a579566a9a1482b20aff8a68f0e4edd99bd28
    # ++Subproject commit 0000000000000000000000000000000000000000
    #
    # To solve that, when a submodule gets in an unmerged state, the confict is
    # fixed automatically by resetting the submodule to upstream's commit.
    all_submodules="$(git config --file .gitmodules --get-regexp 'submodule\..+\.path' | awk '{print($2)}')"
    for changed_file in $(git status --porcelain=v1 | sed -n 's/^UU //p'); do
        if grep -q "^${changed_file}$" <(echo "${all_submodules}"); then
            git reset "${upstream_commit}" -- "${changed_file}"
            echo "pull-upstream: automatically resolved conflict for submodule ${changed_file}"
        fi
    done

    # There could be conflicts between our Cargo.lock and upstream's, as we
    # have our own crates with our own dependencies in the workspace.
    # Automatically resolve any conflict involving Cargo.lock to prefer our own
    # copy of the lockfile rather than upstream's.
    for prefix in "${DIRECTORIES_CONTAINING_LOCKFILES[@]}"; do
        lock="${prefix}Cargo.lock"
        if git status --porcelain=v1 | grep "^UU ${lock}$" >/dev/null; then
            echo "pull-upstream: automatically resolving conflict for ${lock}..."
            git show "${current_branch}:${lock}" > "${lock}"

            # Invoking any Cargo command touching the lockfile will cause the
            # lockfile to be updated. "cargo metadata" is one of the fastest ones.
            # The bootstrap flag is needed as the workspace uses unstable features.
            if ! RUSTC_BOOTSTRAP=1 cargo metadata --format-version=1 "--manifest-path=${prefix}Cargo.toml" >/dev/null; then
                echo "pull-upstream: failed to invoke cargo to update ${lock}, skipping it"
                continue
            fi

            git add "${lock}"
            echo "pull-upstream: automatically resolved conflict for ${lock}"
        fi
    done

    if git diff --diff-filter=U --quiet; then
        # Setting the editor to `true` prevents the actual editor from being open,
        # as in this case we don't want to change the default message.
        GIT_EDITOR="$(which true)" git merge --continue
    elif [[ -n "${EXIT_ON_MERGE_CONFLICT+x}" ]]; then
        echo
        echo "pull-upstream: there are unresolved merge conflicts"
        echo "pull-upstream: resolve the conflicts manually and then run \`git merge --continue\`."
        exit 1
    else
        echo
        echo "pull-upstream: there are unresolved merge conflicts"
        echo "pull-upstream: committing with merge conflict markers in the source"
        echo

        # We do a `git submodule update` ahead of time to ensure the wrong
        # submodule commits are not accidentally added.
        git submodule update --init

        # The person handling the conflict should decide what to do if a file
        # has been deleted on either side of the merge, but doing a `git add .`
        # would mask the conflict (it would simply revert the deletion).
        #
        # To avoid that, we prefix the file with a custom line noting the file
        # had a delete conflict, and the detect-conflict-markers.py script will
        # pick it up and block CI until it's resolved either way.
        handle_deleted_files() {
            marker="$1"
            who="$2"
            for changed_file in $(git status --porcelain=v1 | sed -n "s/^${marker} //p"); do
                sed -i "1s/^/<<<PULL-UPSTREAM>>> file deleted by ${who}, fix the conflict and remove this line\\n/" "${changed_file}"
            done
        }
        handle_deleted_files DU Ferrocene # DU means "deleted by us"
        handle_deleted_files UD Rust      # UD means "deleted by them"

        git add .

        # Setting the editor to `true` prevents the actual editor from being open,
        # as in this case we don't want to change the default message.
        GIT_EDITOR="$(which true)" git merge --continue
    fi
fi

# If there were no changes made since the last pull (i.e. when the diff from the
# previous commit and the pull is empty), remove the empty merge commit and
# exit with a special code to let the automation know it shouldn't open PRs.
if git diff --quiet HEAD^..HEAD; then
    echo "pull-upstream: no changes to pull"
    git reset HEAD^
    exit 42
fi

# Occasionally, changes made upstream require the lockfile to be regenerated,
# otherwise CI with its --locked flag will fail. This is **not** updating the
# versions of the packages we use, but ensuring the lockfile stays consistent.
#
# Whenever the lockfile needs an update (we check that by invoking a Cargo
# command that regenerates the lockfile if needed but doesn't have any side
# effects) we include that in a separate commit.
#
# Note that this is not related to merge conflicts: lockfile merge conflicts
# are automatically fixed by another part of this script.
for prefix in "${DIRECTORIES_CONTAINING_LOCKFILES[@]}"; do
    lock="${prefix}Cargo.lock"
    manifest="${prefix}Cargo.toml"
    echo "pull-upstream: checking whether ${lock} needs to be updated..."
    if ! RUSTC_BOOTSTRAP=1 cargo metadata --format-version=1 "--manifest-path=${manifest}" >/dev/null; then
        echo "pull-upstream: failed to invoke cargo to update ${lock}, skipping it"
        continue
    fi
    if git status --porcelain=v1 | grep "^ M ${lock}$" >/dev/null; then
        git add "${lock}"
        git commit -m "update ${lock} to match ${manifest}"
    fi
    if [ "${upstream_branch}" == "master" ]; then
        echo "pull-upstream: ensure ${lock} has latest semver-compatible crates"
        RUSTC_BOOTSTRAP=1 cargo update --manifest-path "${manifest}"
        if git status --porcelain=v1 | grep "^ M ${lock}$" >/dev/null; then
            git add "${lock}"
            git commit -m "update ${lock} to latest semver-compatible crates"
        fi
    fi
done

# Check whether we can compile bootstrap successfully, which will be used to
# gate on the next few steps.
echo "pull-upstream: checking whether bootstrap can be invoked safely..."
if ./x.py --help > /dev/null; then
    can_invoke_bootstrap=true
else
    can_invoke_bootstrap=false
fi

if [[ "${can_invoke_bootstrap}" = "true" ]]; then
    # We expose additional commands for `x.py` which affects the completions file generation,
    # so we just run the command to regenerate those in case they need updating as this usually
    # does not need manual intervention.
    echo "pull-upstream: checking whether ${GENERATED_COMPLETIONS_DIR} needs to be updated..."
    ./x.py run generate-completions >/dev/null
    if git status --porcelain=v1 | grep "^ M ${GENERATED_COMPLETIONS_DIR}" >/dev/null; then
        git add "${GENERATED_COMPLETIONS_DIR}"
        git commit -m "update ${GENERATED_COMPLETIONS_DIR}"
    fi
else
    echo "pull-upstream: skipped checking whether ${GENERATED_COMPLETIONS_DIR} needs to be updated, due to bootstrap not compiling"
fi

git branch -D "${TEMP_BRANCH}"

echo
echo "You can generate the PR body manually by running:"
echo
echo "    ferrocene/tools/pull-upstream/generate_pr_body.py origin <base-branch> <current-branch>"
echo
