Since you're on 64-bit Pi OS, the target is aarch64. A static musl build is the most reliable route because it doesn't depend on Nix's glibc or loader path.

**1. `.cargo/config.toml`** (a separate file in the project root, not `Cargo.toml`):

```toml
[target.aarch64-unknown-linux-musl]
linker = "aarch64-unknown-linux-musl-gcc"

[env]
CC_aarch64_unknown_linux_musl = "aarch64-unknown-linux-musl-gcc"
```

**2. Enter a shell with the musl cross compiler** (plus cmake in case `aws-lc-sys` wants it):

```bash
nix-shell -p pkgsCross.aarch64-multiplatform-musl.stdenv.cc cmake rustup
```

**3. Build:**

```bash
rustup default stable
rustup target add aarch64-unknown-linux-musl
cargo build --release --target aarch64-unknown-linux-musl
```

**4. Check, copy, run:**

```bash
file target/aarch64-unknown-linux-musl/release/ctl
# should say: ELF 64-bit ... ARM aarch64, statically linked

scp target/aarch64-unknown-linux-musl/release/ctl pi@<pi-address>:~/
ssh pi@<pi-address> ./ctl
```

If it still says "required file not found" after this, run `uname -m` on the Pi. It should print `aarch64`. Anything else means the OS isn't actually 64-bit.

If the build fails in `aws-lc-sys` or `ring`, paste the error and I'll look at it. Those are the two crates most likely to cause trouble when cross-compiling.
