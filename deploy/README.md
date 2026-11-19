# deploy/ — the launcher layer

These files start the panel and bind its hotkeys. They previously lived only in
`~/.config/mango/`, hardcoded `/home/alex` and `/run/user/1000`, so the package
shipped a binary but not the thing that launches it.

Paths derive from `ISLAND_HOME` (default: this repo) and `XDG_RUNTIME_DIR`.

Install:

```sh
cp deploy/rule.conf ~/.config/mango/rule.conf      # or append the layer_rule line
# then source deploy/bind.conf from your Mango config and add
#   deploy/dynamite_autostart.sh
# to your autostart.
```

**The `layer_name` must be `dynamite-rust`** — that is the namespace the binary
sets (`src/main.rs:1932`). An earlier `layer_name:dynamite` matched nothing, so
the no-blur/no-shadow optimisation silently did not apply.
