# Dependency security audit

CI audits both lockfiles with RustSec. The application audit currently ignores
`RUSTSEC-2023-0071`, the timing-side-channel advisory for `rsa` 0.9.

SurrealDB 3.2.3 enables `jsonwebtoken`'s AWS-LC backend on this target. Cargo
still records `jsonwebtoken`'s optional RustCrypto backend in `Cargo.lock`,
which includes `rsa`, but that backend is not enabled in the resolved feature
graph:

```sh
cargo tree -e features -i rsa@0.9.10
```

The command prints no dependency path. Remove the audit exception when
SurrealDB or `jsonwebtoken` no longer records the affected optional package, or
when the `rsa` advisory has a fixed release.

Serenity 0.12.5 is configured with its native-TLS backend because its Rustls
backend is pinned to `rustls-webpki` 0.102.8, which has multiple fixed security
advisories. The bot's direct HTTP clients continue to use current Rustls.
Re-evaluate this choice when a Serenity release moves its WebSocket stack to a
fixed Rustls line.
