# Dependency security audit

CI audits the lockfile with RustSec without exceptions.

Serenity 0.12.5 is configured with its native-TLS backend because its Rustls
backend is pinned to `rustls-webpki` 0.102.8, which has multiple fixed security
advisories. The bot's direct HTTP clients continue to use current Rustls.
Re-evaluate this choice when a Serenity release moves its WebSocket stack to a
fixed Rustls line.
