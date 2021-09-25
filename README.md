# RustCycles

TODO

## Dependencies

TODO

## Fast compiles

You can make the game compile significantly faster and iterate quicker:

### Use nightly, lld and -Zshare-generics

- Run this in project root: `ln -s rust-toolchain-example.toml rust-toolchain.toml; cd .cargo; ln -s config-example.toml config.toml; cd -`
- Reduction from 12 s to 2.5 s

### Prevent rust-analyzer from locking the `target` directory

Add this to your VSCode config (or something similar for your editor):

```json
"rust-analyzer.server.extraEnv": {
    "CARGO_TARGET_DIR": "target-ra"
}
```

Normally, rust-analyzer runs `cargo check` on save which locks `target` so if you switch to a terminal and do `cargo run`, it blocks the build for over a second which is currently a third of the build time. This will make rust-analyzer make use a separate target directory so that it'll never block a build (at the expense of some disk space). Alternatively, you could disable saving when losing focus, disable running check on save or use the terminal inside VSCode.

### On linux, use the `mold` linker

- Reduction from 2.5 s to 2.3 s
- Might not be worth it for now, maybe when the game gets larger

## LICENSE

TODO
