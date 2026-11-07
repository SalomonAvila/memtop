# Paquete RPM de memtop para Fedora (COPR).
#
# Los crates de Rust van incluidos en Source1 (vendor) porque no todas las versiones que
# usa memtop están todavía en Fedora. .copr/Makefile genera Source0 y Source1 a partir
# del repositorio; para compilarlo a mano:
#
#     make -f .copr/Makefile srpm outdir=$PWD/rpm spec=$PWD/packaging/memtop.spec
#     rpmbuild --rebuild rpm/memtop-*.src.rpm

Name:           memtop
Version:        0.1.1
Release:        1%{?dist}
Summary:        System information and resource monitor in a single terminal screen
Summary(es):    Información del sistema y monitor de recursos en una sola pantalla de terminal

# memtop: MIT. data/pci-gpu.ids (incluido en el binario): BSD-3-Clause.
SourceLicense:  MIT AND BSD-3-Clause
# memtop, pci.ids y los crates enlazados en el binario (ver LICENSE.dependencies).
License:        MIT AND BSD-3-Clause AND Zlib AND (Apache-2.0 OR MIT) AND (Apache-2.0 OR BSL-1.0) AND (Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT) AND (MIT OR Unlicense) AND (Zlib OR Apache-2.0 OR MIT)
URL:            https://github.com/SalomonAvila/memtop
Source0:        %{url}/archive/v%{version}/%{name}-%{version}.tar.gz
Source1:        %{name}-%{version}-vendor.tar.xz

ExclusiveArch:  %{rust_arches}

BuildRequires:  cargo-rpm-macros >= 26
# rust-version de Cargo.toml.
BuildRequires:  rust >= 1.95

%global _description %{expand:
memtop combines a system summary (model, OS, kernel, CPU, GPU, memory...) with a
live monitor of CPU, memory, disks, GPU, network and processes, in tabs. It only
reads local information: it needs no root, writes no files and does not use the
network. Optionally shows a random Pokémon if pokemon-colorscripts is installed,
and GPU usage on NVIDIA if nvidia-smi is available.}

%description %{_description}

%description -l es
memtop combina un resumen del equipo (modelo, sistema operativo, kernel, CPU, GPU,
memoria...) con un monitor en vivo de CPU, memoria, discos, GPU, red y procesos, en
pestañas. Solo lee información local: no necesita root, no escribe archivos y no usa
la red. Opcionalmente muestra un Pokémon al azar si pokemon-colorscripts está
instalado, y el uso de la GPU en NVIDIA si nvidia-smi está disponible.

%prep
%autosetup -n %{name}-%{version} -p1 -a1
%cargo_prep -v vendor

%build
%cargo_build
%{cargo_license_summary}
%{cargo_license} > LICENSE.dependencies
%{cargo_vendor_manifest}

%install
%cargo_install

%check
%cargo_test

%files
%license LICENSE THIRD_PARTY_NOTICES.md LICENSE.dependencies cargo-vendor.txt
%doc README.md config.ejemplo.toml
%{_bindir}/memtop

%changelog
* Sat Nov 07 2026 SalomonAvila <salomon.avila2506@gmail.com> - 0.1.1-1
- Nueva versión 0.1.1

* Mon Nov 02 2026 SalomonAvila <salomon.avila2506@gmail.com> - 0.1.0-1
- Primera versión empaquetada
