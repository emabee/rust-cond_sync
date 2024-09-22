# cond_sync

**A thin wrapper around std::sync::CondVar and Mutex that enhances readability when synchronizing threads.**

[![Latest version](https://img.shields.io/crates/v/cond_sync.svg)](https://crates.io/crates/cond_sync)
[![Documentation](https://docs.rs/cond_sync/badge.svg)](https://docs.rs/cond_sync)
[![License](https://img.shields.io/crates/l/cond_sync.svg)](https://github.com/emabee/cond_sync)
[![Build](https://img.shields.io/github/actions/workflow/status/emabee/rust-cond_sync/ci_test.yml?branch=main)](https://github.com/emabee/rust-cond_sync/actions?query=workflow%3ACI)
[![unsafe forbidden](https://img.shields.io/badge/unsafe-forbidden-success.svg)](https://github.com/rust-secure-code/safety-dance/)

## Usage

Add `cond_sync` to the dependencies in your project's `Cargo.toml`:

```toml
[dependencies]
cond_sync = "0.2"
```

## Example

```rust
use cond_sync::{CondSync, Other};

let cond_sync = CondSync::new(0_usize);

for i in 0..5 {
    let cond_sync_t = cond_sync.clone();
    std::thread::spawn(move || {
        // ...initialize...
        cond_sync_t.modify_and_notify(|v| *v += 1, Other::One).unwrap();
        // ...do real work...
    });
}
// wait until all threads are initialized
cond_sync.wait_until(|v| *v == 5).unwrap();

// ...
```

## Dependencies

No dependencies.

## Versions

See the [change log](https://github.com/emabee/cond_sync/blob/master/CHANGELOG.md)
for more details.
