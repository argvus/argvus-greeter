SCRIPT := tools/sh/install.sh

.DEFAULT_GOAL := help

.PHONY: help build install uninstall

help:
	@printf 'targets:\n  make build     build local Arch package with PKGBUILD.local\n  make install   build and install argvus-greeter system-wide (overwrites existing files)\n  make uninstall remove the files installed by "make install"\n'

build:
	@tools/build-local-package.sh

install:
	@./$(SCRIPT) install

uninstall:
	@./$(SCRIPT) uninstall
