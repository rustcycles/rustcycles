<div align="center">
    <h1>RustCycles</h1>
    A fast multiplayer shooter on wheels
</div>
<br />

[![License (AGPL3)](https://img.shields.io/github/license/rustcycles/rustcycles)](https://github.com/rustcycles/rustcycles/blob/master/LICENSE)
[![CI](https://github.com/rustcycles/rustcycles/workflows/CI/badge.svg)](https://github.com/rustcycles/rustcycles/actions)
[![Audit](https://github.com/rustcycles/rustcycles/workflows/audit/badge.svg)](https://rustsec.org/)
[![Dependency status](https://deps.rs/repo/github/rustcycles/rustcycles/status.svg)](https://deps.rs/repo/github/rustcycles/rustcycles)
[![Discord](https://img.shields.io/discord/770013530593689620?label=&logo=discord&logoColor=ffffff&color=7389D8&labelColor=6A7EC2)](https://discord.gg/cXU5HzDXM5)
<!-- These keep getting broken and then they show 0 which looks bad
[![Total lines](https://tokei.rs/b1/github/rustcycles/rustcycles)](https://github.com/rustcycles/rustcycles)
[![Lines of comments](https://tokei.rs/b1/github/rustcycles/rustcycles?category=comments)](https://github.com/rustcycles/rustcycles) -->

<!-- Note to my future OCD: The ideal image width for a github readme is ~838~ 830 pixels. Inspect in firefox and look at Box Model on the Layout tab (value confirmed in gimp). The recommended size for the social preview is higher, likely best to use a different image. -->
<!-- Check https://github.com/topics/tron to make sure it doesn't look blurry. -->
![Gameplay](media/screenshot.png)

RustCycles is a third person arena shooter that's about movement, not aim. You have to be smart and think fast.

_This is just barely a prototype. There's no real gameplay yet, just the engine's default physics and some primitive networking._

Currently RustCycles is the only open source fyrox game which uses networking. If you're also writing a multiplayer game in fyrox, feel free to ping me on Fyrox's or RustCycles' discord to exchange notes and ideas.

Multiplayer shooters are large and complex projects and 80% of the code is not game specific. I am looking to collaborate with anyone making a similar game. The plan is to identify generic parts and build RustCycles into a FPS-specific fyrox "subengine" that provides a solid foundation for first/third person shooters so everyone can focus on the parts that make their game unique.

## Building

There are no prebuilt binaries yet, you have to build it yourself.

- Install [git LFS](https://git-lfs.github.com/) **before** cloning this repo.

- Install dependencies if on Linux (debian): `sudo apt install libasound2-dev libudev-dev pkg-config xorg-dev libxcb-shape0-dev libxcb-xfixes0-dev libxkbcommon-dev libfontconfig1-dev`
<!-- libfontconfig1-dev is not needed on CI for some reason but I couldn't compile without it on Kubuntu 22.04 -->

- After that, just use `cargo run`.
  - No need to use `--release` it should run fast enough in debug mode because deps are optimized even in debug mode (see Cargo.toml).

## Development

### Fast compiles (optional)

You can make the game compile _significantly_ faster and iterate quicker:

#### Use nightly, lld and -Zshare-generics

- Run this in project root: `ln -s rust-toolchain-example.toml rust-toolchain.toml; cd .cargo; ln -s config-example.toml config.toml; cd -`
- Reduction from 12 s to 2.5 s

#### Prevent rust-analyzer from locking the `target` directory

If you're using RA with `clippy` instead of `check`, add this to your VSCode config (or something similar for your editor):

```json
"rust-analyzer.server.extraEnv": {
    "CARGO_TARGET_DIR": "target/ra"
}
```

Explanation: Normally, if rust-analyzer runs `cargo clippy` on save, it locks `target` so if you switch to a terminal and do `cargo run`, it blocks the build for over a second which is currently a third of the build time. This will make rust-analyzer make use a separate target directory so that it'll never block a build at the expense of slightly more disk space (but not double since most files don't seem to be shared between cargo and RA). Alternatively, you could disable saving when losing focus, disable running check on save or use the terminal inside VSCode to build RustCycles.

#### On linux, use the `mold` linker

- `~/your/path/to/mold -run cargo build`
- Reduction from 2.5 s to 2.3 s
- Might not be worth it for now (you need to compile it yourself), maybe when the game gets larger

### Check formatting on commit (optional)

Enable extra checks before every commit: copy/symlink `pre-commit-example` to `pre-commit` and run `git config core.hooksPath git-hooks`. It gets checked on CI anyway, this just catches issues faster.

## LICENSE

[AGPL-v3](LICENSE) or newer
