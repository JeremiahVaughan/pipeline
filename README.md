# Pipeline MVC skeleton

A minimal Rust MVC workspace that uses crate boundaries to keep rebuilds fast.

## Crates

- `crates/db`: SQLite initialization and migration runner.
- `crates/model`: Domain models and SQL for interacting with SQLite.
- `crates/controller`: Controller logic that coordinates between models and views using the models.
- `crates/view`: Rendering helpers for presenting models.
- `crates/app`: Binary entrypoint wiring the layers together.

## Getting started

```bash
cargo run -p app
```

The sample output renders a single seeded user profile through the controller and view layers.

### Database

- SQLite migrations live in `crates/db/migrations` and are applied on startup.
- SQLite writes are guarded by a mutex in the model layer to keep write queries serialized.
