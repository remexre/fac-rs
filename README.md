[![Build Status](https://travis-ci.org/Arnavion/fac-rs.svg?branch=master)](https://travis-ci.org/Arnavion/fac-rs)

An API and manager for Factorio mods. It's a Rust clone of https://github.com/mickael9/fac

The API functionality is split up into separate reusable crates:

- `factorio-mods-local`: API to interface with the local Factorio installation.
- `factorio-mods-web`: API to interface with https://mods.factorio.com/
- `factorio-mods-common`: Common types and functionality used by the other crates.
- `derive-struct`: A helper crate for easily deriving structs.
