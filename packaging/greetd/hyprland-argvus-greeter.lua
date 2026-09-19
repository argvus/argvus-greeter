-- The Lua and .conf variants intentionally express the same minimal session
-- policy for installations that select either Hyprland configuration format.
-- Minimal Hyprland configuration for Argvus Greeter.
-- This intentionally avoids loading the user's full Argvus session before login.

hl.monitor({
  output = "",
  mode = "preferred",
  position = "auto",
  scale = "auto",
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
  -- Start through the terminal policy wrapper so Kitty inherits the active
  -- ARGVUS theme while applying the Greeter-only tab restrictions.
  hl.exec_cmd("/usr/bin/argvus-greeter-tui")
end)
