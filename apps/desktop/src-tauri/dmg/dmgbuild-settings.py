# dmgbuild settings for the Cortex installer, used by scripts/desktop/release-macos.sh:
#   dmgbuild -s dmgbuild-settings.py -D app=<Cortex.app> -D background=<background.png> Cortex <out.dmg>
# dmgbuild writes the window layout (.DS_Store) itself, so no Finder scripting is needed.
# It picks up background@2x.png next to background.png for Retina screens.
import os.path

app = defines["app"]  # noqa: F821 (dmgbuild provides `defines`)
app_name = os.path.basename(app)

format = "UDZO"
filesystem = "HFS+"
files = [app]
symlinks = {"Applications": "/Applications"}

background = defines["background"]  # noqa: F821
# 660x400 points; the app on the left, Applications on the right, over the arrow.
window_rect = ((200, 120), (660, 400))
default_view = "icon-view"
show_status_bar = False
show_tab_view = False
show_toolbar = False
show_pathbar = False
show_sidebar = False
icon_size = 128
text_size = 13
icon_locations = {app_name: (180, 200), "Applications": (480, 200)}
