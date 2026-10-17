BINARY      := openghub
PREFIX      := $(HOME)/.local
BINDIR      := $(PREFIX)/bin
DESKTOP_DIR := $(PREFIX)/share/applications
ICON_DIR    := $(PREFIX)/share/icons/hicolor/512x512/apps
UDEV_RULE   := /etc/udev/rules.d/99-openghub.rules

.PHONY: build publish test install uninstall run clean

build:
	dotnet build OpenGHub.slnx

# The single-file trimmed self-contained binary the packaging uses.
publish:
	dotnet publish src/OpenGHub.App -p:PublishProfile=linux-x64 -o bin
	mv bin/OpenGHub.App bin/$(BINARY)

test:
	dotnet build

install: publish
	mkdir -p $(BINDIR)
	install -m 0755 bin/$(BINARY) $(BINDIR)/$(BINARY)
	mkdir -p $(DESKTOP_DIR)
	install -m 0644 install/openghub.desktop $(DESKTOP_DIR)/openghub.desktop
	mkdir -p $(ICON_DIR)
	install -m 0644 Icon.png $(ICON_DIR)/$(BINARY).png
	# hicolor needs an index.theme to be a usable theme root; gtk-update-icon-cache -t only touches an existing one, so we ship ours.
	install -m 0644 install/hicolor-index.theme $(PREFIX)/share/icons/hicolor/index.theme
	sudo install -m 0644 install/99-openghub.rules $(UDEV_RULE)
	sudo udevadm control --reload-rules
	sudo udevadm trigger
	-command -v gtk-update-icon-cache >/dev/null 2>&1 && gtk-update-icon-cache -f $(PREFIX)/share/icons/hicolor 2>/dev/null
	@echo ""
	@echo "Installed to $(BINDIR)/$(BINARY)."
	@echo "Unplug and replug your Logitech device so the new udev rule applies."

uninstall:
	rm -f $(BINDIR)/$(BINARY)
	rm -f $(DESKTOP_DIR)/openghub.desktop
	rm -f $(ICON_DIR)/$(BINARY).png
	sudo rm -f $(UDEV_RULE)
	sudo udevadm control --reload-rules

run: build
	dotnet run --project src/OpenGHub.App

clean:
	rm -rf bin