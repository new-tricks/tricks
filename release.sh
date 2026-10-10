#!/usr/bin/env bash
# Releases New Tricks in two steps, because main only changes through reviewed
# pull requests:
#
#   ./release.sh <version | major | minor | patch>
#       From an up-to-date main: checks formatting, lints and tests, bumps the
#       version (Cargo.toml and the extension's package.json) and names
#       CHANGELOG.md's Unreleased section for it on a release-<version>
#       branch, and opens its pull request, which has the release notes and
#       the commits.
#
#   ./release.sh tag
#       Once that is merged: checks CHANGELOG.md has the version and CI passed
#       on main, tags v<version>, follows the release workflow (GitHub
#       release, binaries, VSIX packages, Homebrew formula, crates.io), checks
#       the release has every platform's binary, the tap points at the tag and
#       crates.io has the version.
#
#   ./release.sh notes [<version | major | minor | patch>]
#       Prints what the release pull request would say, changing nothing.
#
#   ./release.sh changelog [<version>]
#       Prints the version's section of CHANGELOG.md (Cargo.toml's version if
#       none is given), the GitHub release's notes; fails if it has none.
#
#   ./release.sh changelog --named <version> [<date>]
#       Prints CHANGELOG.md as <bump> writes it for <version>, changing
#       nothing.
#
# CHANGELOG.md is Keep a Changelog 1.1.0 (https://keepachangelog.com/en/1.1.0/).
#
# Needs git, cargo, python3 and gh (logged in, with push access).
set -euo pipefail
cd "$(dirname "$0")"

repo=new-tricks/tricks
tap=new-tricks/homebrew-tap
# What the release workflow builds a tricks-<target>.tar.gz for.
targets=(aarch64-apple-darwin x86_64-apple-darwin x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu
    x86_64-pc-windows-msvc aarch64-pc-windows-msvc)

die() { echo "release.sh: $*" >&2; exit 1; }
say() { echo "==> $*"; }

current_version() { sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1; }

# The section of CHANGELOG.md headed `## [<name>]` (a version, or
# Unreleased), up to the next or the link definitions, without its heading or
# the blank lines around it. Each paragraph and list item is put on one line,
# because GitHub shows a release's and a pull request's line breaks as they
# are.
changelog() {
    awk -v name="$1" '
        function out(s) { printf "%s%s\n", gap, s; gap = ""; text = 1 }
        function flush() { if (line != "") out(line); line = "" }
        /^## / { if (on) exit; on = ($0 == "## [" name "]" || index($0, "## [" name "] - ") == 1); next }
        /^\[[^ ]+\]: / { if (on) exit; next }
        !on { next }
        /^```/ { flush(); fence = !fence; out($0); next }
        fence { out($0); next }
        !NF { flush(); if (text) gap = gap "\n"; next }
        line == "" || /^ *(#+|[-*>|]|[0-9]+\.) / { flush(); line = $0; next }
        { sub(/^ +/, ""); line = line " " $0 }
        END { flush() }
    ' CHANGELOG.md
}

# <version>'s release notes: its section of CHANGELOG.md, which it must have.
release_section() {
    local text
    text=$(changelog "$1")
    [ -n "$text" ] || die "CHANGELOG.md has no section for $1 (## [$1] - YYYY-MM-DD); './release.sh <bump>' makes it from Unreleased"
    echo "$text"
}

# CHANGELOG.md with what Unreleased has headed `## [<version>] - <date>`,
# under a new, empty Unreleased, and the links moved: <version> compares the
# tag Unreleased compared from with v<version>, and Unreleased v<version> with
# HEAD. Fails without the Unreleased heading or link.
named_changelog() {
    awk -v version="$1" -v date="$2" -v url="https://github.com/$repo/compare/" '
        !head && $0 == "## [Unreleased]" { print; print ""; print "## [" version "] - " date; head = 1; next }
        !link && index($0, "[Unreleased]: " url) == 1 && /\.\.\.HEAD$/ {
            from = substr($0, length("[Unreleased]: " url) + 1)
            print "[Unreleased]: " url "v" version "...HEAD"
            print "[" version "]: " url substr(from, 1, length(from) - length("...HEAD")) "...v" version
            link = 1
            next
        }
        { print }
        END { exit !(head && link) }
    ' CHANGELOG.md
}

unnamed() { die "CHANGELOG.md needs '## [Unreleased]' and '[Unreleased]: https://github.com/$repo/compare/<tag>...HEAD'"; }

# The extension's package.json and package-lock.json at <version>: the
# package's own "version", not its dependencies'.
bump_extension() {
    python3 - "$1" <<'PY'
import json, sys
version = sys.argv[1]
for path, keys in (("extension/package.json", [[]]), ("extension/package-lock.json", [[], ["packages", ""]])):
    with open(path) as f:
        data = json.load(f)
    for key in keys:
        node = data
        for k in key:
            node = node[k]
        node["version"] = version
    with open(path, "w") as f:
        json.dump(data, f, indent=2, ensure_ascii=False)
        f.write("\n")
PY
}

extension_version() { python3 -c 'import json; print(json.load(open("extension/package.json"))["version"])'; }

on_clean_main() {
    [ "$(git branch --show-current)" = main ] || die "not on main"
    [ -z "$(git status --porcelain)" ] || die "the working tree has changes"
    git fetch -q origin
    [ "$(git rev-parse HEAD)" = "$(git rev-parse origin/main)" ] || die "main isn't origin/main (git pull)"
}

next_version() {
    local current=$1 wanted=$2 major minor patch
    IFS=. read -r major minor patch <<<"$current"
    case $wanted in
        major) echo "$((major + 1)).0.0" ;;
        minor) echo "$major.$((minor + 1)).0" ;;
        patch) echo "$major.$minor.$((patch + 1))" ;;
        [0-9]*.[0-9]*.[0-9]*) echo "$wanted" ;;
        *) die "a version (1.2.3), major, minor or patch, not '$wanted'" ;;
    esac
}

prepare() {
    on_clean_main
    local current version last branch today
    current=$(current_version)
    version=$(next_version "$current" "$1")
    branch=release-$version
    git rev-parse -q --verify "refs/tags/v$version" >/dev/null && die "v$version is tagged already"
    [ -n "$(changelog Unreleased)" ] || die "CHANGELOG.md has nothing under Unreleased: say there what $version changes"
    today=$(date -u +%Y-%m-%d)
    named_changelog "$version" "$today" >/dev/null || unnamed
    last=$(git describe --tags --abbrev=0 --match 'v*' 2>/dev/null || true)

    say "checking main"
    cargo fmt --check
    cargo clippy --release --all-targets -- -D warnings
    TRICKS_NO_GH=1 cargo test --release --locked

    say "bumping $current -> $version on $branch"
    git checkout -q -b "$branch"
    sed -i.bak "s/^version = \"$current\"/version = \"$version\"/" Cargo.toml && rm Cargo.toml.bak
    cargo update -q --workspace
    [ "$(current_version)" = "$version" ] || die "couldn't bump Cargo.toml"
    bump_extension "$version"
    [ "$(extension_version)" = "$version" ] || die "couldn't bump extension/package.json"
    named_changelog "$version" "$today" >CHANGELOG.md.new || unnamed
    mv CHANGELOG.md.new CHANGELOG.md
    release_section "$version" >/dev/null
    git commit -q -am "Release $version"
    git push -q -u origin "$branch"

    say "opening the pull request"
    gh pr create -R "$repo" --base main --head "$branch" --title "Release $version" \
        --body "$(release_notes "$version" "$last" "$version")"
    echo
    echo "Once it is merged: ./release.sh tag"
}

# What the release pull request says: CHANGELOG.md's <section> (the
# version's, or Unreleased before it is named), the commits since <last>, and
# whether extension/CHANGELOG.md, which the Marketplace shows, has the
# version too.
release_notes() {
    local version=$1 last=$2 section=$3 range=HEAD
    [ -n "$last" ] && range="$last..HEAD"
    echo "Bumps the version to $version. Once this is merged, \`./release.sh tag\` tags \`v$version\`, which runs the release workflow."
    echo
    echo "## Release notes"
    echo
    echo "CHANGELOG.md's section for $version, which the GitHub release says; edit it here."
    echo
    changelog "$section"
    echo
    echo "## Commits since ${last:-the start}"
    git log --no-merges --format='- %s' "$range" | grep -v "^- Release " || echo "- (none)"
    echo
    echo "## VS Code extension"
    echo
    if grep -qxF "## $version" extension/CHANGELOG.md; then
        echo "extension/CHANGELOG.md has a section for $version."
    elif [ -n "$last" ] && git diff --quiet "$last" HEAD -- extension; then
        echo "extension/ is unchanged since $last; the extension is republished at $version with the new binary."
    else
        echo "**extension/CHANGELOG.md has no section for $version**, and the extension changed since ${last:-the start}: add one here if users notice the change in the editor."
    fi
}

tag() {
    on_clean_main
    local version run
    version=$(current_version)
    git rev-parse -q --verify "refs/tags/v$version" >/dev/null && die "v$version is tagged already; bump the version first"
    release_section "$version" >/dev/null
    [ "$(extension_version)" = "$version" ] || die "extension/package.json is $(extension_version), not $version"

    say "checking CI on main ($(git rev-parse --short HEAD))"
    local conclusion
    conclusion=$(gh run list -R "$repo" --workflow ci.yml --commit "$(git rev-parse HEAD)" --json conclusion --jq '.[0].conclusion // "none"')
    [ "$conclusion" = success ] || die "CI on main is '$conclusion', not success"

    say "tagging v$version"
    git tag -a "v$version" -m "v$version"
    git push -q origin "v$version"

    say "following the release workflow"
    sleep 10
    run=$(gh run list -R "$repo" --workflow release.yml --branch "v$version" --limit 1 --json databaseId --jq '.[0].databaseId')
    [ -n "$run" ] || die "no release run for v$version yet; see https://github.com/$repo/actions"
    gh run watch "$run" -R "$repo" --exit-status

    say "checking the release's binaries"
    local assets target
    assets=$(gh release view "v$version" -R "$repo" --json assets --jq '.assets[].name')
    for target in "${targets[@]}"; do
        grep -qxF "tricks-$target.tar.gz" <<<"$assets" || die "v$version has no tricks-$target.tar.gz"
        grep -qxF "tricks-$target.tar.gz.sha256" <<<"$assets" || die "v$version has no tricks-$target.tar.gz.sha256"
    done

    say "checking the tap"
    gh api "repos/$tap/contents/Formula/tricks.rb" --jq .content | base64 --decode |
        grep -qF "https://github.com/$repo/archive/refs/tags/v$version.tar.gz" ||
        die "$tap's tricks formula doesn't point at v$version"

    say "checking crates.io"
    curl -fsS -A "tricks release.sh (https://github.com/$repo)" "https://crates.io/api/v1/crates/tricks/$version" >/dev/null ||
        die "crates.io doesn't have tricks $version"
    gh release view "v$version" -R "$repo" --json url --jq .url
    echo "New Tricks $version is out: brew upgrade tricks, cargo install tricks --locked, or tricks upgrade"
}

case ${1:-} in
    tag) tag ;;
    notes)
        last=$(git describe --tags --abbrev=0 --match 'v*' 2>/dev/null || true)
        release_notes "$(next_version "$(current_version)" "${2:-patch}")" "$last" Unreleased
        ;;
    changelog)
        if [ "${2:-}" = --named ]; then
            [ -n "${3:-}" ] || die "changelog --named <version> [<date>]"
            named_changelog "$3" "${4:-$(date -u +%Y-%m-%d)}" || unnamed
        else
            release_section "${2:-$(current_version)}"
        fi
        ;;
    "" | -h | --help) sed -n '2,32p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) prepare "$1" ;;
esac
