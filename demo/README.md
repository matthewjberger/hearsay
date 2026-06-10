# demo

A process-per-window desktop shell built on Bevy, egui, and [hearsay](..).

Run it from the repository root:

```
just run-demo
```

The first instance hosts a hearsay broker and runs its window as a client of it. "New Window" spawns another process of the same executable, supervised by the broker, which connects to the same bus. Windows coordinate over broker topics: layouts are assigned and collected as messages, so a project file can describe many windows while every window stays a plain single-window app.

Widgets are registered through the `widgets!` macro and talk to the bus through a `WidgetRpc` handle. The `Template` widget demonstrates the full surface: pub/sub, binary payloads, file dialogs, notifications, and modals.

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).
