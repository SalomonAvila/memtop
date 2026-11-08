# Builds the memtop SRPM for COPR ("make_srpm" build method).
#
# COPR runs it as root inside a chroot with network access:
#     make -f .copr/Makefile srpm outdir=<output directory> spec=<path to the .spec>
#
# Creates the two sources that packaging/memtop.spec needs from the current commit:
#   - memtop-VERSION.tar.gz         source code (git archive)
#   - memtop-VERSION-vendor.tar.xz  Rust crates (cargo vendor)

ROOT    := $(abspath $(dir $(lastword $(MAKEFILE_LIST)))..)
NAME    := memtop
VERSION := $(shell sed -n 's/^version = "\(.*\)"/\1/p' $(ROOT)/Cargo.toml | head -n1)
outdir  ?= $(ROOT)/rpm
spec    ?= $(ROOT)/packaging/$(NAME).spec
TMP     := $(outdir)/.vendor-tmp

.PHONY: srpm deps

# Inside COPR (root) installs what is needed; on your machine it assumes you already have it.
deps:
	@if [ "$$(id -u)" = 0 ]; then \
		dnf -y install cargo git-core rpm-build tar xz && \
		git config --global --add safe.directory $(ROOT); \
	fi

srpm: deps
	@test -n "$(VERSION)" || { echo "Could not find the version in Cargo.toml"; exit 1; }
	@grep -q '^Version: *$(VERSION)$$' $(spec) || { \
		echo "The .spec version does not match Cargo.toml ($(VERSION))"; exit 1; }
	mkdir -p $(outdir)
	rm -rf $(TMP)
	git -C $(ROOT) archive --format=tar.gz --prefix=$(NAME)-$(VERSION)/ \
		-o $(outdir)/$(NAME)-$(VERSION).tar.gz HEAD
	cd $(ROOT) && cargo vendor --locked $(TMP)/vendor > /dev/null
	tar -C $(TMP) -cJf $(outdir)/$(NAME)-$(VERSION)-vendor.tar.xz vendor
	rm -rf $(TMP)
	rpmbuild -bs --define "_sourcedir $(outdir)" --define "_srcrpmdir $(outdir)" $(spec)
