#!/usr/bin/env bash
# Stage fwpanel-service and its system assets under a caller-owned directory.
#
# This performs NO privileged operations: it builds the service and copies
# files into <destdir>/usr/... for review. Installing them into the system is
# a separate, explicit owner action; see README.md ("Host service
# installation") for the exact commands.
set -euo pipefail

destdir="${1:?usage: stage-service.sh <destdir>}"
root="$(cd "$(dirname "$0")/.." && pwd)"

cargo build --release --manifest-path "$root/Cargo.toml" -p fwpanel-service

install -D -m 0755 \
  "$root/target/release/fwpanel-service" \
  "$destdir/usr/libexec/fwpanel-service"
install -D -m 0644 \
  "$root/packaging/systemd/fwpanel-service.service" \
  "$destdir/usr/lib/systemd/system/fwpanel-service.service"
install -D -m 0644 \
  "$root/packaging/dbus/io.github.singh_gur.Fwpanel1.service" \
  "$destdir/usr/share/dbus-1/system-services/io.github.singh_gur.Fwpanel1.service"
install -D -m 0644 \
  "$root/packaging/dbus/io.github.singh_gur.Fwpanel1.conf" \
  "$destdir/usr/share/dbus-1/system.d/io.github.singh_gur.Fwpanel1.conf"
install -D -m 0644 \
  "$root/packaging/polkit/io.github.singh_gur.fwpanel.policy" \
  "$destdir/usr/share/polkit-1/actions/io.github.singh_gur.fwpanel.policy"

echo "Staged fwpanel-service assets under: $destdir"
echo "Nothing has been installed. To install, an administrator copies these"
echo "five files to the same paths under / — see README.md."
