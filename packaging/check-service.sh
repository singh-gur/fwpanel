#!/usr/bin/env bash
# Read-only checks against the installed fwpanel system service.
# Performs no mutations and triggers no hardware writes.
set -euo pipefail

NAME="io.github.singh_gur.Fwpanel1"
PATH_OBJ="/io/github/singh_gur/Fwpanel1"
IFACE="io.github.singh_gur.Fwpanel1"

echo "== D-Bus introspection (expect only the five application methods"
echo "   plus standard interfaces) =="
busctl --system introspect "$NAME" "$PATH_OBJ" "$IFACE"

echo
echo "== GetServiceInfo (active local session: no prompt expected) =="
busctl --system call "$NAME" "$PATH_OBJ" "$IFACE" GetServiceInfo

echo
echo "== systemd unit state =="
systemctl status fwpanel-service.service --no-pager || true
