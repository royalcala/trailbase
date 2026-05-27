# Attribution

This project (syntrix-core) is a derivative work based on
[TrailBase](https://github.com/trailbaseio/trailbase) by trailbaseio,
licensed under the Open Software License v3.0 (OSL-3.0).

The original TrailBase codebase provided the foundational infrastructure:
SQLite integration, Axum HTTP server, authentication system, Records API,
WASM runtime, realtime subscriptions, and admin UI scaffold.

syntrix-core extends and diverges from TrailBase with:
- Multi-org architecture: separate SQLite database per organization (`org_<slug>.db`)
- Queue system: dedicated `queue.db` with `_queue_job`, `_queue_job_attempt`, `_queue_dead_letter` tables
- Extended schema management: `system/` vs `app/` separation with declarative CLI
- Admin API extensions: queue job listing and statistics endpoints
- Extended schema export CLI supporting all 5 system databases (main, org, session, logs, queue)
- TypeScript and Rust SDK extensions for queue admin methods

## Original License

The original work is licensed under OSL-3.0. See [LICENSE](./LICENSE).

Per OSL-3.0 section 1(c), all distributed copies of this Derivative Work
are also licensed under OSL-3.0.

## Original Authors

TrailBase contributors — https://github.com/trailbaseio/trailbase/graphs/contributors

## Changes

All modifications and additions made in the `syntrix-main` branch and beyond
are authored by the Syntrix team (royalcala and contributors).
