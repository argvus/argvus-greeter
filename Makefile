SCRIPT := tools/sh/install.sh

.DEFAULT_GOAL := help

.PHONY: help install uninstall

help:
	@printf 'targets:\n  make install   build and install argvus-greeter system-wide (overwrites existing files)\n  make uninstall remove the files installed by "make install"\n'

install:
	@./$(SCRIPT) install

uninstall:
	@./$(SCRIPT) uninstall
