# Pipeline MVC skeleton

A minimal Rust MVC workspace that uses crate boundaries to keep rebuilds fast.

## Crates

- `crates/db`: SQLite initialization and migration runner.
- `crates/model`: Domain models and SQL for interacting with SQLite.
- `crates/controller`: Controller logic that coordinates between models and views using the models.
- `crates/view`: Rendering helpers for presenting models.
- `crates/app`: Binary entrypoint wiring the layers together.
- `crates/config`: Loads TOML configuration into a globally accessible struct.

## Getting started

```bash
cargo run -p app
```

The sample output renders a single seeded user profile through the controller and view layers.

### Database

- SQLite migrations live in `crates/db/migrations` and are applied on startup.
- SQLite writes are guarded by a mutex in the model layer to keep write queries serialized.

### Configuration

- Edit `config/example.toml` to set application options such as `database_path`.
- Configuration is loaded once at startup and exposed globally for convenience.

### Custom htmx over websockets

This app uses a small `custom_htmx.js` shim that mirrors the familiar htmx attributes, but all interactions travel over the websocket (`static/ws.js`).

The main quirk is `hx-patch`: instead of issuing an HTTP request, its value becomes the first token in the websocket message. The rest of the message is built from the closest parent form's input fields, in DOM order, and each item is delimited by `:`.

Example

```html
<form>
  <input hx-patch="search_services" hx-trigger="input" />
</form>
```

If the user types `fire`, the client sends:

```
search_services:fire
```

If the form has multiple inputs, each input value is appended in order:

```
search_services:first_value:second_value
```
