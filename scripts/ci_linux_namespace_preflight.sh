#!/usr/bin/env bash
# Shared AppArmor policy and bind-mapping admission for both Linux CI hosts.
set -euo pipefail
fail() {
    echo "ci-host-provision: core.001a.invalid: $*" >&2
    exit 1
}
[ "$#" = 1 ] || fail "namespace preflight requires one provisioning root"
work_root=$1
[ -d "$work_root" ] && [ ! -L "$work_root" ] ||
    fail "namespace preflight requires an existing private directory"

profile="$work_root/usr.bin.bwrap"
printf '%s\n' \
    'abi <abi/4.0>,' \
    '' \
    'include <tunables/global>' \
    '' \
    '/usr/bin/bwrap flags=(default_allow) {' \
    '  userns,' \
    '}' > "$profile"
sudo -n /usr/sbin/apparmor_parser --replace "$profile"

probe_root="$work_root/probe"
mkdir -p -- "$probe_root"
host_identity=$(stat -c '%d:%i' "$probe_root")
mapped_identity=$(/usr/bin/bwrap --die-with-parent --bind / / \
    --dev-bind /dev /dev --proc /proc --ro-bind /sys /sys \
    --bind "$probe_root" /tmp -- /usr/bin/stat -c '%d:%i' /tmp) ||
    fail "Bubblewrap namespace probe failed"
[ "$mapped_identity" = "$host_identity" ] ||
    fail "Bubblewrap bind mapping failed read-back"
