-- Minimal Hyprland configuration for Argvus Greeter.
-- This intentionally avoids loading the user's full Argvus session before login.

hl.monitor({
  output = "",
  mode = "preferred",
  position = "auto",
  scale = 1,
})

hl.env("XDG_CURRENT_DESKTOP", "Hyprland")
hl.env("XDG_SESSION_DESKTOP", "argvus-greeter")
hl.env("XDG_SESSION_TYPE", "wayland")
hl.env("GDK_BACKEND", "wayland")

hl.config({
  general = {
    gaps_in = 0,
    gaps_out = 0,
    border_size = 0,
  },

  decoration = {
    rounding = 0,
    shadow = {
      enabled = false,
    },
    blur = {
      enabled = false,
    },
  },

  animations = {
    enabled = false,
  },

  input = {
    kb_layout = "us",
    follow_mouse = 0,
  },

  misc = {
    disable_hyprland_logo = true,
    disable_splash_rendering = true,
    force_default_wallpaper = 0,
  },
})

hl.on("hyprland.start", function()
  hl.exec_cmd("sh -lc '/usr/bin/argvus-greeter; hyprctl dispatch exit'")
end)
