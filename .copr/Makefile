# Genera el SRPM de memtop para COPR (método de compilación "make_srpm").
#
# COPR lo ejecuta como root dentro de un chroot con red:
#     make -f .copr/Makefile srpm outdir=<carpeta de salida> spec=<ruta del .spec>
#
# Crea las dos fuentes que pide packaging/memtop.spec a partir del commit actual:
#   - memtop-VERSION.tar.gz         código (git archive)
#   - memtop-VERSION-vendor.tar.xz  crates de Rust (cargo vendor)

RAIZ    := $(abspath $(dir $(lastword $(MAKEFILE_LIST)))..)
NOMBRE  := memtop
VERSION := $(shell sed -n 's/^version = "\(.*\)"/\1/p' $(RAIZ)/Cargo.toml | head -n1)
outdir  ?= $(RAIZ)/rpm
spec    ?= $(RAIZ)/packaging/$(NOMBRE).spec
TMP     := $(outdir)/.vendor-tmp

.PHONY: srpm deps

# Dentro de COPR (root) instala lo necesario; en tu equipo da por hecho que ya lo tienes.
deps:
	@if [ "$$(id -u)" = 0 ]; then \
		dnf -y install cargo git-core rpm-build tar xz && \
		git config --global --add safe.directory $(RAIZ); \
	fi

srpm: deps
	@test -n "$(VERSION)" || { echo "No se encontró la versión en Cargo.toml"; exit 1; }
	@grep -q '^Version: *$(VERSION)$$' $(spec) || { \
		echo "La versión del .spec no coincide con la de Cargo.toml ($(VERSION))"; exit 1; }
	mkdir -p $(outdir)
	rm -rf $(TMP)
	git -C $(RAIZ) archive --format=tar.gz --prefix=$(NOMBRE)-$(VERSION)/ \
		-o $(outdir)/$(NOMBRE)-$(VERSION).tar.gz HEAD
	cd $(RAIZ) && cargo vendor --locked $(TMP)/vendor > /dev/null
	tar -C $(TMP) -cJf $(outdir)/$(NOMBRE)-$(VERSION)-vendor.tar.xz vendor
	rm -rf $(TMP)
	rpmbuild -bs --define "_sourcedir $(outdir)" --define "_srcrpmdir $(outdir)" $(spec)
