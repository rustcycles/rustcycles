<div align="center">
    <h1>RustCycles</h1>
    A fast multiplayer shooter on wheels
</div>
<br />

[![Dependency status](https://deps.rs/repo/github/rustcycles/rustcycles/status.svg)](https://deps.rs/repo/github/rustcycles/rustcycles)
[![Discord](https://img.shields.io/discord/770013530593689620?label=&logo=discord&logoColor=ffffff&color=7389D8&labelColor=6A7EC2)](https://discord.gg/cXU5HzDXM5)
[![Total lines](https://tokei.rs/b1/github/rustcycles/rustcycles)](https://github.com/rustcycles/rustcycles)
[![Lines of comments](https://tokei.rs/b1/github/rustcycles/rustcycles?category=comments)](https://github.com/rustcycles/rustcycles)

<!-- Note to my future OCD: The ideal image width for github is 838 pixels -->
<!-- Also check https://github.com/topics/tron to make sure it doesn't look blurry -->
![Gameplay](media/screenshot.png)

RustCycles is a third person shooter that's about movement, not aim. You have to be smart and think fast.

_**This is not even a prototype yet. But don't be disappointed, bookmark it and come back a few months later ;)**_

## Development

Install [git LFS](https://git-lfs.github.com/) before cloning this repo.

After that, just use `cargo run`.

### Fast compiles (optional)

You can make the game compile significantly faster (around 2 seconds) and iterate quicker:

#### Use nightly, lld and -Zshare-generics

- Run this in project root: `ln -s rust-toolchain-example.toml rust-toolchain.toml; cd .cargo; ln -s config-example.toml config.toml; cd -`
- Reduction from 12 s to 2.5 s

#### Prevent rust-analyzer from locking the `target` directory

Add this to your VSCode config (or something similar for your editor):

```json
"rust-analyzer.server.extraEnv": {
    "CARGO_TARGET_DIR": "target-ra"
}
```

Normally, rust-analyzer runs `cargo check` on save which locks `target` so if you switch to a terminal and do `cargo run`, it blocks the build for over a second which is currently a third of the build time. This will make rust-analyzer make use a separate target directory so that it'll never block a build (at the expense of some disk space). Alternatively, you could disable saving when losing focus, disable running check on save or use the terminal inside VSCode to build RustCycles.

#### On linux, use the `mold` linker

- `~/your/path/to/mold -run cargo build`
- Reduction from 2.5 s to 2.3 s
- Might not be worth it for now (you need to compile it yourself), maybe when the game gets larger

### WASM builds

Sometimes `wasm-pack` triggers a full rebuild unnecessarily so it's best to build wasm in a separate target directory:

```bash
CARGO_TARGET_DIR=target-wasm wasm-pack build --target web --dev
```

Note: I am not sure if this is a bug but the cause seems to be that using the fast compiles config causes `RUSTFLAGS` to differ between native and WASM builds so if you alternate between those, they will always rebuild everything. For debugging it, these commands might be useful:

```bash
CARGO_LOG=cargo::core::compiler::fingerprint=trace wasm-pack build --target web --dev
CARGO_LOG=cargo::core::compiler::fingerprint=trace cargo build
```

## LICENSE

[AGPL-v3](agpl-3.0.txt) or newer
