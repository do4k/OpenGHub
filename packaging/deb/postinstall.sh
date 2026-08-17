#!/bin/sh
# Reload udev rules after install so LogiTux's HID access rule takes
# effect without waiting for a reboot. Best-effort: a headless or
# container environment may have no running udev, which is not a failure.
set -e

if command -v udevadm >/dev/null 2>&1; then
	udevadm control --reload-rules || true
	udevadm trigger || true
fi

# Refresh the icon caches so the launcher picks up logitux.png.
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
	for d in /usr/share/icons/hicolor "$HOME/.local/share/icons/hicolor"; do
		[ -d "$d" ] && gtk-update-icon-cache -f "$d" >/dev/null 2>&1 || true
	done
fi

exit 0