A Multiwii Serial Protocol (MSP) implementation for Rust
===========================================

[![Build Status](https://github.com/msp-rs/multiwii_serial_protocol_v2.rs/workflows/Rust/badge.svg)](https://github.com/msp-rs/multiwii_serial_protocol_v2.rs/actions)
[![Documentation](https://docs.rs/multiwii_serial_protocol_v2/badge.svg)](https://docs.rs/multiwii_serial_protocol_v2)

## Introduction

This is a fork of https://github.com/hashmismatch/multiwii_serial_protocol.rs!

An incomplete implementation of the MSP2 protocol, with some Cleanflight, Betaflight and iNav extensions. Allows one to implement a flight controller that can connect to the Cleanflight or Baseflight configurator.

# Installation

MSP is available on crates.io and can be included in your Cargo enabled project like this:

```toml
[dependencies]
multiwii_serial_protocol_2 = "0.1.12`"
```

Then include it in your code like this:

```rust
extern crate multiwii_serial_protocol_2;
```

License: MIT OR Apache-2.0
