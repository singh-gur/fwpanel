#!/usr/bin/env bash
# Install the fwpanel host service and the unprivileged Flatpak GUI.
#
# Supported families: RPM-based (Fedora/RHEL and derivatives) and Debian-based
# (Debian/Ubuntu and derivatives). Both build from source; there are no
# published artifacts.
#
#   service  RPM family: built into an RPM and installed with dnf.
#            Debian family: the five files are installed directly, because no
#            .deb packaging exists (same paths the RPM would own).
#   flatpak  Identical everywhere: a local --user build and install. Contains
#            only the unprivileged GUI; it can never install the service.
#
# Privileged steps are limited to the service: package-manager installs and
# copying the five root-owned files. Everything Flatpak-related stays --user.
# Run as an ordinary user; the script calls sudo only where it must.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

FLATPAK_ID="io.github.singh_gur.fwpanel"
FLATPAK_MANIFEST="$root/packaging/flatpak/io.github.singh-gur.fwpanel.yml"
SERVICE_UNIT="fwpanel-service.service"
RPM_NAME="fwpanel-service"

# The five root-owned files the service consists of, as "dest|mode". They are
# staged under a destdir by stage-service.sh, so each destination path doubles
# as the staged relative path. Keep in sync with stage-service.sh and the spec.
SERVICE_FILES=(
  "/usr/libexec/fwpanel-service|0755"
  "/usr/lib/systemd/system/fwpanel-service.service|0644"
  "/usr/share/dbus-1/system-services/io.github.singh_gur.Fwpanel1.service|0644"
  "/usr/share/dbus-1/system.d/io.github.singh_gur.Fwpanel1.conf|0644"
  "/usr/share/polkit-1/actions/io.github.singh_gur.fwpanel.policy|0644"
)

do_service=1
do_flatpak=1
uninstall=0
assume_yes=0

usage() {
  cat <<'EOF'
Usage: packaging/install.sh [options]

Installs the fwpanel host service and the Flatpak GUI on Fedora- and
Ubuntu-based distributions, building both from source.

Options:
  --service-only   Only the privileged host service
  --flatpak-only   Only the unprivileged Flatpak GUI
  --uninstall      Remove what this script installs (not build deps/toolchains)
  -y, --yes        Do not prompt for confirmation
  -h, --help       Show this help

Requires cargo (rustup) for the service. The Flatpak build needs no host
toolchain; Rust and Node come from the sandboxed SDK extensions.
EOF
}

while [ $# -gt 0 ]; do
  case "$1" in
    --service-only) do_flatpak=0 ;;
    --flatpak-only) do_service=0 ;;
    --uninstall) uninstall=1 ;;
    -y | --yes) assume_yes=1 ;;
    -h | --help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
  shift
done

info() { printf '\n== %s\n' "$*"; }
warn() { printf 'warning: %s\n' "$*" >&2; }
die() {
  printf 'error: %s\n' "$*" >&2
  exit 1
}

# Confirm before anything that touches the system. --yes answers everything.
confirm() {
  [ "$assume_yes" -eq 1 ] && return 0
  local reply
  read -r -p "$1 [y/N] " reply
  case "$reply" in
    [yY] | [yY][eE][sS]) return 0 ;;
    *) return 1 ;;
  esac
}

have() { command -v "$1" >/dev/null 2>&1; }

# ---------------------------------------------------------------- environment

[ "$(id -u)" -ne 0 ] || die "run as an ordinary user, not root: the Flatpak is
installed per-user, and the script escalates with sudo only where required."

[ "$(uname -m)" = "x86_64" ] || die "unsupported architecture $(uname -m);
fwpanel targets x86_64 (Framework Laptop 13 AMD Ryzen AI 300)."

[ -r /etc/os-release ] || die "cannot read /etc/os-release; unsupported system."
# shellcheck disable=SC1091
. /etc/os-release

case " ${ID:-} ${ID_LIKE:-} " in
  *" fedora "* | *" rhel "* | *" centos "*) family=rpm ;;
  *" debian "* | *" ubuntu "*) family=deb ;;
  *) die "unsupported distribution '${ID:-unknown}'; this script handles
Fedora/RHEL- and Debian/Ubuntu-based systems." ;;
esac

echo "Detected ${PRETTY_NAME:-$ID} (${family}-based)"

have sudo || die "sudo is required for the privileged service steps."

if [ "$family" = rpm ]; then
  pkgmgr=dnf
  have dnf || pkgmgr=yum
  have "$pkgmgr" || die "neither dnf nor yum found."
fi

# --------------------------------------------------------------- dependencies

# Deliberately minimal. The service links only pure-Rust crates, so it needs a
# C linker and nothing else; the GUI's webkit2gtk/librsvg stack is provided by
# the Flatpak runtime, not the host.
missing_packages() {
  local -a want=() missing=()
  if [ "$do_service" -eq 1 ]; then
    if [ "$family" = rpm ]; then
      want+=(gcc rpm-build)
    else
      want+=(build-essential)
    fi
  fi
  if [ "$do_flatpak" -eq 1 ]; then
    want+=(flatpak flatpak-builder)
  fi

  local p
  for p in "${want[@]}"; do
    if [ "$family" = rpm ]; then
      rpm -q "$p" >/dev/null 2>&1 || missing+=("$p")
    else
      [ "$(dpkg-query -W -f='${db:Status-Status}' "$p" 2>/dev/null || true)" = "installed" ] ||
        missing+=("$p")
    fi
  done
  if [ "${#missing[@]}" -gt 0 ]; then
    printf '%s\n' "${missing[@]}"
  fi
}

ensure_packages() {
  local -a missing=()
  mapfile -t missing < <(missing_packages)
  if [ "${#missing[@]}" -eq 0 ]; then
    echo "All required distribution packages are already installed."
    return 0
  fi

  info "Missing distribution packages"
  printf '  %s\n' "${missing[@]}"
  if [ "$family" = rpm ]; then
    echo "Command: sudo $pkgmgr install ${missing[*]}"
  else
    echo "Command: sudo apt-get install ${missing[*]}"
  fi

  confirm "Install these packages now?" ||
    die "dependencies not installed; re-run after installing them yourself."

  if [ "$family" = rpm ]; then
    sudo "$pkgmgr" install -y "${missing[@]}"
  else
    sudo apt-get update
    sudo apt-get install -y "${missing[@]}"
  fi
}

# Toolchains are user-managed (rustup/nvm), so never install them silently.
require_cargo() {
  have cargo && return 0
  die "cargo not found on PATH. Install Rust with rustup, then re-run:
  curl --proto '=https' --tlsv1.2 https://sh.rustup.rs -sSf | sh
If rustup is installed already, source your profile or add ~/.cargo/bin to PATH."
}

# ------------------------------------------------------------------- service

install_service_rpm() {
  info "Building the host-service RPM"
  local topdir="$root/stage/rpm"
  cargo build --release --manifest-path "$root/Cargo.toml" -p fwpanel-service
  rpmbuild -bb \
    --define "_topdir $topdir" \
    --define "fwpanel_bin $root/target/release/fwpanel-service" \
    --define "fwpanel_repo $root" \
    "$root/packaging/rpm/fwpanel-service.spec"

  local rpm_file
  rpm_file="$(find "$topdir/RPMS" -name "${RPM_NAME}-*.rpm" -type f -printf '%T@ %p\n' |
    sort -rn | head -1 | cut -d' ' -f2-)"
  [ -n "$rpm_file" ] || die "rpmbuild produced no ${RPM_NAME} package under $topdir/RPMS."

  info "Installing $(basename "$rpm_file")"
  echo "This installs root-owned files and requires administrator rights."
  confirm "Install the host service now?" || die "service installation declined."
  # Handles both first install and upgrade over an existing version.
  sudo "$pkgmgr" install -y "$rpm_file"
}

install_service_files() {
  info "Building and staging the host service"
  # Staged inside the gitignored stage/ directory, so a failed run leaves an
  # inspectable tree rather than an orphaned temp directory.
  local staged="$root/stage/install-root"
  rm -rf "$staged"
  "$root/packaging/stage-service.sh" "$staged" >/dev/null

  info "Installing the host service"
  echo "The following root-owned files will be written:"
  local entry dest mode
  for entry in "${SERVICE_FILES[@]}"; do
    IFS='|' read -r dest mode <<<"$entry"
    printf '  %s (%s)\n' "$dest" "$mode"
  done

  confirm "Install these files now?" || die "service installation declined."

  for entry in "${SERVICE_FILES[@]}"; do
    IFS='|' read -r dest mode <<<"$entry"
    sudo install -D -o root -g root -m "$mode" "${staged}${dest}" "$dest"
  done
  rm -rf "$staged"

  # D-Bus activated: reload the managers, never enable it at boot.
  sudo systemctl daemon-reload
  sudo systemctl reload dbus 2>/dev/null || true
}

install_service() {
  require_cargo
  if [ "$family" = rpm ]; then
    install_service_rpm
  else
    install_service_files
  fi
  echo "Host service installed. It is D-Bus activated and is not enabled at boot."
}

remove_service() {
  info "Removing the host service"
  sudo systemctl stop "$SERVICE_UNIT" 2>/dev/null || true

  if [ "$family" = rpm ] && rpm -q "$RPM_NAME" >/dev/null 2>&1; then
    sudo "$pkgmgr" remove -y "$RPM_NAME"
  else
    # Covers the Debian path and any Fedora install done without the RPM.
    local entry dest removed=0
    for entry in "${SERVICE_FILES[@]}"; do
      IFS='|' read -r dest _ <<<"$entry"
      if [ -e "$dest" ]; then
        sudo rm -f "$dest"
        removed=1
      fi
    done
    [ "$removed" -eq 1 ] || echo "No installed service files found."
  fi

  sudo systemctl daemon-reload
  sudo systemctl reset-failed "$SERVICE_UNIT" 2>/dev/null || true
  echo "Host service removed."
}

# ------------------------------------------------------------------- flatpak

install_flatpak() {
  have flatpak-builder || die "flatpak-builder not found after dependency setup."

  # The manifest builds git-tracked files from HEAD of this checkout, so
  # uncommitted work silently will not reach the Flatpak.
  if [ -n "$(git -C "$root" status --porcelain 2>/dev/null || true)" ]; then
    warn "the working tree has uncommitted changes; the Flatpak builds from
git HEAD, so those changes will NOT be included."
    confirm "Continue building from HEAD anyway?" || die "Flatpak build declined."
  fi

  info "Configuring the user Flathub remote"
  flatpak remote-add --if-not-exists --user \
    flathub https://dl.flathub.org/repo/flathub.flatpakrepo

  info "Building and installing the Flatpak GUI (user installation)"
  echo "The GNOME runtime/SDK and Rust/Node SDK extensions download first;"
  echo "the build itself then runs offline. This takes a while."
  flatpak-builder --user --install-deps-from=flathub --install --force-clean \
    "$root/stage/flatpak/build" "$FLATPAK_MANIFEST"
  echo "Flatpak installed. Run it with: flatpak run $FLATPAK_ID"
}

remove_flatpak() {
  info "Removing the Flatpak GUI"
  if have flatpak && flatpak info --user "$FLATPAK_ID" >/dev/null 2>&1; then
    flatpak uninstall --user -y "$FLATPAK_ID"
    echo "Flatpak removed."
  else
    echo "Flatpak $FLATPAK_ID is not installed for this user."
  fi
}

# ---------------------------------------------------------------------- main

if [ "$uninstall" -eq 1 ]; then
  if [ "$do_flatpak" -eq 1 ]; then remove_flatpak; fi
  if [ "$do_service" -eq 1 ]; then remove_service; fi
  info "Uninstall complete"
  echo "Build dependencies and toolchains were left untouched."
  exit 0
fi

ensure_packages
if [ "$do_service" -eq 1 ]; then install_service; fi
if [ "$do_flatpak" -eq 1 ]; then install_flatpak; fi

info "Done"
if [ "$do_service" -eq 1 ]; then
  echo "Verify the service with read-only checks: just check-service"
fi
if [ "$do_flatpak" -eq 1 ]; then
  echo "Launch the GUI:                          flatpak run $FLATPAK_ID"
  echo "Inspect the sandbox permissions:         just check-flatpak"
fi
