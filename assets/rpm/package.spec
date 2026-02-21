Name:           preboot-oxide
Version:        %{version}
Release:        1%{?dist}
Summary:        PXE boot server for networks where a DHCP server already exists

License:        MIT
URL:            https://github.com/alexculea/Preboot-Oxide

%description
PXE boot server for networks where a DHCP server already exists and should be kept separate.

%build
# No build step needed, binary is pre-built

%install
mkdir -p %{buildroot}/bin
mkdir -p %{buildroot}/usr/lib/systemd/system
cp %{_sourcedir}/preboot-oxide %{buildroot}/bin/
cp %{_sourcedir}/preboot-oxide.service %{buildroot}/usr/lib/systemd/system/

%files
/bin/preboot-oxide
/usr/lib/systemd/system/preboot-oxide.service

%changelog
* Mon Feb 21 2026 Alex Culea <16594531+alexculea@users.noreply.github.com> - %{version}-1
- Initial RPM package
