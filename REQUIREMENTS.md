# Requirements

The recommended cross-platform environment is the Dev Container in
[`arachne/.devcontainer`](./arachne/.devcontainer). A native Rust workflow is
also supported on Linux, macOS, and Windows.

The commands in the artifact READMEs are valid in POSIX shells and Windows
PowerShell unless a block is explicitly labeled for one shell.

## Recommended: Dev Container

The Dev Container provides a Debian environment containing:

- Rust and Cargo 1.93
- Git and the system build tools supplied by the Rust base image
- Python 3 for the optional ModelSet experiment
- A release build of the `arachne` CLI installed in `/usr/local/bin`
- the Rust Analyzer and TOML extensions when opened with Visual Studio Code

The host does need:

- Docker Desktop
- [Visual Studio Code](https://code.visualstudio.com/) with the
  [Dev Containers extension](https://marketplace.visualstudio.com/items?itemName=ms-vscode-remote.remote-containers),
  or the Dev Container CLI
- Network access for the base image, Debian packages, crates.io, and Git
  dependencies during the first image build
- Several gigabytes of free storage for the image and Cargo artifacts

### Start with Visual Studio Code

1. Open the artifact's `arachne` directory in Visual Studio Code
2. Open the command palette
3. Select **Dev Containers: Reopen in Container**
4. Wait for the image build and workspace initialization to finish

The container mounts the complete artifact at `/workspaces/artifact` and opens
`/workspaces/artifact/arachne`:

- Arachne commands can be run immediately in the opened terminal
- The packaged projects are available under `../generated`
- The local Moirai source is available under `../moirai`
- The artifact-level README and status files are available under `..`

Verify the environment:

```sh
rustc --version
cargo --version
python3 --version
arachne --version
```

The Rust commands should report version 1.93, and `arachne --version` should
print `arachne 0.3.0`.

### Start with the Dev Container CLI

From the artifact root:

```sh
devcontainer up --workspace-folder arachne
devcontainer exec --workspace-folder arachne bash
```

The second command opens a shell in the Arachne workspace. Use `exit` to leave
it.

### ModelSet in the container

The ModelSet dataset must be visible inside the container. The simplest option
is to extract it somewhere under the artifact root, then pass that container
path to `modelset_coverage.py`. Alternatively, add a Docker bind mount for an
existing dataset directory.

## Native installation

For a native workflow, install:

- Git
- Rust 1.93, the artifact's evaluation baseline, using
  [rustup](https://rustup.rs/)
- Cargo, installed with Rust
- a system C/C++ linker and build tools
- network access for the first Cargo build
- Python 3.10 or later only for the ModelSet experiment

### Windows

The recommended native Windows configuration is the default Rust MSVC
toolchain:

1. Install Rust with `rustup`
2. Install Visual Studio 2022 or Build Tools for Visual Studio 2022
3. Select the Desktop development with C++ workload
4. Ensure the MSVC x64/x86 build tools and a Windows 10 or Windows 11 SDK are
   selected
5. Restart the terminal after installation

See the
[official Rust MSVC prerequisites](https://rust-lang.github.io/rustup/installation/windows-msvc.html).

### MacOS

Install the Xcode command-line tools if a system linker is not already
available:

```sh
xcode-select --install
```

Both Apple Silicon and Intel systems can use the standard Rust host toolchain.

### Linux

Install a C/C++ linker and standard build tools in addition to Rust. On
Debian/Ubuntu:

```sh
sudo apt-get update
sudo apt-get install build-essential git
```

Use the equivalent packages for other distributions.

## Cargo dependency

Arachne itself uses crates.io dependencies and its local workspace crates.
Generated projects use the pinned Moirai Git revision documented in the README.
Pass `--moirai-path ../moirai` to generation to use the local Moirai workspace
instead. A first build needs network access unless every dependency is already cached.

## Hardware and storage

No specialized hardware is required for building Arachne or running the
packaged scenario tests.

However, the complete ModelSet experiment is substantially heavy:

- At least 10 GB of free disk space is recommended for temporary generated
  projects and Cargo artifacts
- Running the script on the +5,000 Ecore models in the ModelSet dataset can take a long time, depending on the host's CPU and I/O performance
- A small "smoke test" is provided for a quick installation check

## Additional ModelSet requirements

- Python 3.10 or later
- [ModelSet v0.9.4](https://github.com/modelset/modelset-dataset/releases/tag/v0.9.4),
  downloaded and extracted locally
- Network access

(On native Windows, invoke Python as `py -3`. On Linux, macOS, and inside the
Dev Container, use `python3`)
