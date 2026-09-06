#!/usr/bin/env bash
set -euo pipefail

fail() {
    echo "stdlib-payload: invalid: $*" >&2
    exit 1
}

[ "$#" -eq 1 ] || fail "usage: $0 <seen-stdlib-*.tar.gz>"
archive="$1"
[ -f "$archive" ] && [ ! -L "$archive" ] ||
    fail "archive is missing or unsafe"

member_count=0
listing="$(mktemp "${TMPDIR:-/tmp}/seen-stdlib-members.XXXXXX")" ||
    fail "could not create member listing"
trap 'rm -f -- "$listing"' EXIT
tar -tzf "$archive" > "$listing" || fail "archive could not be listed"
while IFS= read -r member; do
    member_count=$((member_count + 1))
    case "$member" in
        /*|../*|*/../*|*/..) fail "unsafe member: $member" ;;
        seen_std|seen_std/|seen_std/src|seen_std/src/) continue ;;
        seen_std/src/*) ;;
        *) fail "unexpected member root: $member" ;;
    esac
    case "/$member/" in
        *.tmp.*|*/build/*|*/target/*|*/.seen/*)
            fail "prohibited generated member: $member"
            ;;
    esac
done < "$listing"

[ "$member_count" -gt 0 ] || fail "archive is empty"
echo "PASS: canonical standalone stdlib payload"
