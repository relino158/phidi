# README.md

Phidi is a lightweight development environment with first-class support for agentic workflows, built on top of the awesome [Lapce IDE](https://github.com/lapce/lapce). UI in [Floem](https://github.com/lapce/floem).

Other cool projects that inspired features include:
* [GitNexus](https://github.com/abhigyanpatwari/GitNexus)
* [oh-my-pi](https://github.com/can1357/oh-my-pi)
* [OpenClaw](https://github.com/openclaw/openclaw)

This project is an early WIP.

## Features

* Built-in [LSP](https://microsoft.github.io/language-server-protocol/) support to give you intelligent code features such as: completion, diagnostics and code actions.
* Modal editing support as first class citizen (Vim-like, and toggleable)
* Built-in remote development support inspired by [VSCode Remote Development](https://code.visualstudio.com/docs/remote/remote-overview). Enjoy the benefits of a "local" experience, and seamlessly gain the full power of a remote system.
* Plugins can be written in programming languages that can compile to the [WASI](https://wasi.dev/) format (C, Rust, [AssemblyScript](https://www.assemblyscript.org/))
* Built-in terminal, so you can execute commands in your workspace, without leaving Lapce.

## License

Apache 2.0.

The original Lapce intellectual property belongs to [Lapdev](https://lap.dev) and is released under the Apache License Version 2. You can find a copy of their license text [here](https://github.com/lapce/lapce/blob/master/LICENSE). All changes and additions from the Phidi project are also released under an Apache 2.0 license; a copy [here](LICENSE).

This project and its creator are not associated with or endorsed by Lapdev in any way.