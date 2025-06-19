<a id="readme-top"></a>

<div align="center">
  <h3 align="center">containr</h3>

  <p align="center">Minimal OCI Runtime</p>
</div>

<details>
  <summary>Table of Contents</summary>
  <ol>
    <li><a href="#about-the-project">About The Project</a></li>
    <li>
      <a href="#getting-started">Getting Started</a>
      <ul>
        <li><a href="#building">Building</a></li>
      </ul>
    </li>
    <li><a href="#usage">Usage</a></li>
    <li><a href="#roadmap">Roadmap</a></li>
    <li><a href="#contact">Contact</a></li>
    <li><a href="#acknowledgments">Acknowledgments</a></li>
  </ol>
</details>

## About The Project

This is a toy project to learn more about the OCI runtime specification and
Linux userland APIs.

<p align="right">(<a href="#readme-top">back to top</a>)</p>

## Getting Started

### Building

`containr` is packaged as a Nix flake so you can build it with:

```sh
nix build
```

or with Cargo if you're not using Nix:

```sh
cargo build --release
```

<p align="right">(<a href="#readme-top">back to top</a>)</p>

## Usage

You can specify the runtime for podman using an option:

```sh
podman --runtime=/path/to/containr run -it debian bash
```

Theoretically, you should be able to use `docker` instead of `podman` but I
haven't tested it.

<p align="right">(<a href="#readme-top">back to top</a>)</p>

## Roadmap

- [x] Start a container process
- [x] Set up pseudoterminal when required
- [x] Pivot root according to given spec
- [ ] Set up namespaces according to given spec
- [ ] Set up mounts according to given spec

<p align="right">(<a href="#readme-top">back to top</a>)</p>

## Contact

Veselin Ivanov - me@veselyn.com

Project Link: [https://github.com/veselyn/containr](https://github.com/veselyn/containr)

<p align="right">(<a href="#readme-top">back to top</a>)</p>

## Acknowledgments

- [youki](https://github.com/youki-dev/youki)

<p align="right">(<a href="#readme-top">back to top</a>)</p>
