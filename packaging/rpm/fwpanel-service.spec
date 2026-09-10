# fwpanel host-service RPM (helper-only package).
#
# Built from the workspace release output; rpmbuild must be invoked with:
#   rpmbuild -bb --define "_topdir <dir>" \
#     --define "fwpanel_bin <repo>/target/release/fwpanel-service" \
#     --define "fwpanel_repo <repo>" packaging/rpm/fwpanel-service.spec
# The `just build-service-rpm` recipe wires the paths.
Name:           fwpanel-service
Version:        0.1.0
Release:        2%{?dist}
Summary:        Privileged fwpanel host service for Framework laptops

License:        MIT
ExclusiveArch:  x86_64
Requires:       polkit
Requires:       systemd
Requires:       dbus
# Protocol compatibility capability consumed by the GUI package. The protocol
# major is embedded in the capability NAME because tauri-bundler's rpm depends
# are name-only (Dependency::any) and cannot express a versioned require.
Provides:       fwpanel-service-api-1
Provides:       fwpanel-service-api = 1

%description
Privileged system service exposing Framework laptop hardware status and the
charge-limit control over the system D-Bus. Authorizes every caller through
polkit; installed files are root-owned. The GUI (fwpanel) connects to this
service; this package owns only the service's files.

%install
install -D -m 0755 "%{fwpanel_bin}" \
    %{buildroot}/usr/libexec/fwpanel-service
install -D -m 0644 "%{fwpanel_repo}/packaging/systemd/fwpanel-service.service" \
    %{buildroot}/usr/lib/systemd/system/fwpanel-service.service
install -D -m 0644 \
    "%{fwpanel_repo}/packaging/dbus/io.github.singh_gur.Fwpanel1.service" \
    %{buildroot}/usr/share/dbus-1/system-services/io.github.singh_gur.Fwpanel1.service
install -D -m 0644 "%{fwpanel_repo}/packaging/dbus/io.github.singh_gur.Fwpanel1.conf" \
    %{buildroot}/usr/share/dbus-1/system.d/io.github.singh_gur.Fwpanel1.conf
install -D -m 0644 \
    "%{fwpanel_repo}/packaging/polkit/io.github.singh_gur.fwpanel.policy" \
    %{buildroot}/usr/share/polkit-1/actions/io.github.singh_gur.fwpanel.policy
install -D -m 0644 "%{fwpanel_repo}/LICENSE" \
    %{buildroot}/usr/share/licenses/fwpanel-service/LICENSE

%files
%license /usr/share/licenses/fwpanel-service/LICENSE
/usr/libexec/fwpanel-service
/usr/lib/systemd/system/fwpanel-service.service
/usr/share/dbus-1/system-services/io.github.singh_gur.Fwpanel1.service
/usr/share/dbus-1/system.d/io.github.singh_gur.Fwpanel1.conf
/usr/share/polkit-1/actions/io.github.singh_gur.fwpanel.policy

# D-Bus/systemd activated: never enabled at boot, so only reload the managers.
%post
/usr/bin/systemctl daemon-reload
/usr/bin/systemctl reload dbus 2>/dev/null || true

%postun
/usr/bin/systemctl daemon-reload

%changelog
* Thu Sep 10 2026 fwpanel maintainer <singh-gur@localhost> - 0.1.0-2
- Upgrade-path test release (no content changes).

* Thu Sep 10 2026 fwpanel maintainer <singh-gur@localhost> - 0.1.0-1
- Initial service package (protocol 1; GetServiceInfo, GetPower, GetPorts,
  GetInputDeck, SetChargeLimit).
