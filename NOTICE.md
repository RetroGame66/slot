# Notice

## This is a modified fork

This repository is a fork of **[BrandonKowalski/slot](https://github.com/BrandonKowalski/slot)** —
"a bespoke, GBA-only frontend for the Anbernic RG SP".

Everything originally in the project — the code, the artwork, the design, the interface's English
voice — is the work of **Brandon T. Kowalski**. It is used here under the MIT License (see
[`LICENSE`](LICENSE)). This fork adds to and modifies that work; it does not relicense it.

**Fork maintained by [@RetroGame66](https://github.com/RetroGame66).**

The fork's own documentation is [`README.md`](README.md). Upstream's README is reproduced
unmodified, and introduced as such, at [`README.upstream.md`](README.upstream.md).

> **中文说明**
>
> 本仓库是 [BrandonKowalski/slot](https://github.com/BrandonKowalski/slot) 的修改版（fork）。
> 原作由 **Brandon T. Kowalski** 开发，采用 MIT 许可；本仓库在其基础上做了修改，
> 修改部分由 [@RetroGame66](https://github.com/RetroGame66) 完成。
> 本仓库**不是**上游官方版本，与上游作者、Anbernic，以及 mGBA、gpSP 等第三方项目
> 均无隶属、赞助或背书关系。

## Baseline

This fork diverges from upstream commit
[`4345cb8bdb`](https://github.com/BrandonKowalski/slot/commit/4345cb8bdb) — *Merge branch
'feat/core-picker-board'*, 2026-09-11T03:10:00Z.

Upstream has continued past that point. Pulling `upstream/main` forward is the intended way to
stay current; the fork's own work belongs on a branch, leaving `main` free to track upstream
without carrying a permanent divergence.

## What this fork changes

**75 files, +5,929 / −385** — eleven new files, 64 modified. The full list, by feature rather than by
file, is in [`CHANGES.md`](CHANGES.md). In short, it adds: a Chinese interface whose typeface is
read off the card rather than baked into the binary; a pinyin letter ring over the shelf; cart
faces built off the frame loop with a bounded texture pool; selectable panel-mask presets and a
colour-correction stage, four new `SELECT` chords and all; a twenty-step backlight; audio latency
profiles; per-cart cheat codes with an on-device browser; button remapping; and GB/GBC cartridge
artwork (assets only, not wired up).

## Licensing of what is not ours

The frontend, and every change made in this fork, are **MIT**. Three things the project ships or
references are not, and keep their own terms:

| Thing | Licence | Text |
|---|---|---|
| `mgba_libretro` | MPL-2.0 | [`licenses/mgba-MPL-2.0.txt`](licenses/mgba-MPL-2.0.txt) |
| `gpsp_libretro` | GPL-2.0 | [`licenses/gpsp-GPL-2.0.txt`](licenses/gpsp-GPL-2.0.txt) |
| Bundled typefaces | SIL OFL 1.1 / MIT | the `.txt` files beside them in [`crates/slot-ui/assets/`](crates/slot-ui/assets) |
| The pinyin readings in `crates/slot-store/src/pinyin.rs` | MIT | [pypinyin](https://github.com/mozillazg/python-pinyin) — the table is generated from that project's data by `tools/gen_pinyin.py` | |

`slot` loads both emulator cores as libretro shared objects and never links against or modifies
either — which is why neither licence reaches the frontend's own code. Upstream's own reasoning
about the cores, including the corresponding-source arrangement GPL-2.0 section 3(a) asks for, is
recorded in [`licenses/README.md`](licenses/README.md). A binary distribution built from this tree
inherits that obligation; `taskfile.yml`'s `dist:device` task satisfies it by copying `licenses/`
into the shipped tree alongside the cores.

## No affiliation

This fork is unofficial and unsupported. It is not endorsed by, affiliated with, or supported by
the upstream author, by Anbernic, or by the maintainers of mGBA, gpSP, BaseOS or AGS-102. Bug
reports about the changes listed in `CHANGES.md` belong here, not upstream — please do not send
them to the original project.
