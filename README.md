<div align="center">
    <h1>RustCycles</h1>
    A fast multiplayer shooter on wheels
</div>
<br />

[![GitHub](https://img.shields.io/badge/github-rustcycles/rustcycles-8da0cb?logo=github)](https://github.com/rustcycles/rustcycles)
[![License (AGPLv3)](https://img.shields.io/github/license/rustcycles/rustcycles)](https://github.com/rustcycles/rustcycles/blob/master/LICENSE)
[![CI](https://github.com/rustcycles/rustcycles/workflows/CI/badge.svg)](https://github.com/rustcycles/rustcycles/actions)
[![Audit](https://github.com/rustcycles/rustcycles/workflows/audit/badge.svg)](https://rustsec.org/)
[![Dependency status](https://deps.rs/repo/github/rustcycles/rustcycles/status.svg)](https://deps.rs/repo/github/rustcycles/rustcycles)
[![Discord](https://img.shields.io/badge/-Discord-7389d8?logo=discord&label=&logoColor=ffffff&labelColor=6A7EC2)](https://discord.gg/cXU5HzDXM5)
<!-- These keep getting broken and then they show 0 which looks bad, comment out when that happens. -->
[![Total lines](https://tokei.rs/b1/github/rustcycles/rustcycles)](https://github.com/rustcycles/rustcycles)
[![Lines of comments](https://tokei.rs/b1/github/rustcycles/rustcycles?category=comments)](https://github.com/rustcycles/rustcycles)

<!-- To avoid keeping the file in the repo forever, use either the social preview or upload it to a dummy github issue (AFAIK the issue doesn't even need to be submitted and it'll still be hosted forever). -->
![Spectating](https://github.com/rustcycles/rustcycles/assets/4079823/f6ad566c-54f0-49c0-9a2a-5019e908f09e)
![Gameplay](https://github.com/rustcycles/rustcycles/assets/4079823/5411df7a-6d31-482b-b3a0-ab3256f5280e)
<!-- When updating this, also update https://fyrox.rs/games.html -->

RustCycles is a third person arena shooter that's about movement, not aim. You have to be smart and think fast.

_This is just barely a prototype. There's no real gameplay yet, just the engine's default physics and some primitive networking._

Currently RustCycles is the only open source Fyrox game which uses networking. If you're also writing a multiplayer game in Fyrox, feel free to ping me on Fyrox's or RustCycles' discord to exchange notes and ideas.

Multiplayer shooters are large and complex projects and 80% of the code is not game specific. I am looking to collaborate with anyone making a similar game. The plan is to identify generic parts and build RustCycles into a FPS-specific Fyrox "subengine" that provides a solid foundation for first/third person shooters so everyone can focus on the parts that make their game unique.

## Building

There are no prebuilt binaries and no web version yet, you have to build the game yourself.

- RustCycles uses git submodules for its assets. To clone the repo, use `git clone --recurse-submodules git@github.com:rustcycles/rustcycles.git`. If you already cloned it without submodules, use `git submodule update --init --recursive`.

- If on linux, install dependencies (debian example): `sudo apt install libasound2-dev libudev-dev pkg-config xorg-dev libxcb-shape0-dev libxcb-xfixes0-dev libxkbcommon-dev libfontconfig1-dev`
<!-- libfontconfig1-dev is not needed on CI for some reason but I couldn't compile without it on Kubuntu 22.04 -->

- After that, just use `cargo run`.
  - No need to use `--release` it should run fast enough in debug mode because deps are optimized even in debug mode (see Cargo.toml).

## Development

Currently using git submodules for assets because GitHub's LFS has a tiny 1 GB per month bandwidth limit that's not sufficient already with just a couple MB of data and won't scale. Committing assets into the main repo would cause its size to grow irreversibly. A separate repo as a submodule allows us to keep the main repo small without overwriting history. The data repo can then be either squashed or replaced with a fresh one if the history gets too large.

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
