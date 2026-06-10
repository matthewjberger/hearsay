# Architecture

The demo is a desktop dashboard shell built on Bevy, bevy_egui, egui_tiles, and [hearsay](https://github.com/matthewjberger/hearsay). Every window is its own process. The first process hosts a hearsay broker and connects to it as a client; every additional window is a spawned copy of the same executable that connects to the same broker. All cross-window coordination is plain topic-based pub/sub.

## Process model

`WindowRole::detect` decides what a process is at startup: if the `HEARSAY_BROKER` environment variable is present the process is a `Child` and connects to that address, otherwise it is the `Primary` and hosts the broker on `127.0.0.1:9612` before connecting its own client to it.

"New Window" on the primary sends `BrokerServiceMessage::SpawnWindow`, which calls `hearsay::spawn_app` with the current executable. The broker supervises the child, injects `HEARSAY_BROKER`, and kills it when the broker is dropped. A child window's "New Window" publishes `shell/window/request-spawn`, which the primary handles by spawning. If a child loses its broker connection, it exits, so killing the primary tears down every window through either path: graceful shutdown drops the broker and kills supervised children, and a hard kill severs their connections.

## Broker runtime (`services/broker.rs`)

Bevy systems are synchronous and hearsay is async, so the broker lives on a dedicated thread running a tokio runtime. The Bevy side holds a `BrokerLink` resource with an unbounded command channel in and an unbounded event channel out:

- `RuntimeCommand`: Publish, PublishBytes, Subscribe, Unsubscribe, SpawnWindow
- `RuntimeEvent`: Connected, Disconnected, Failed, Inbound

The runtime loop selects between draining commands and `hearsay::next_message`. On disconnect it falls back to polling `hearsay::is_connected` (the hearsay client auto-reconnects and re-subscribes on its own) and emits Connected when the link returns.

`SubscriptionRegistry` reference-counts topics by widget id on the Bevy side: subscribes always forward to the broker, but a topic is only unsubscribed when its last widget leaves. `WidgetRemoved` sweeps all of a closed widget's topics.

## Message bus (`api.rs`)

`Message` is the single envelope for everything: connection status, inbound topics, broker commands, filesystem commands and results, modals, notifications, project messages, and tile messages. Systems and widgets emit `MessageBusEvent::RouteMessage(message)`; `route_messages_to_services` fans each message out to the typed event of the owning service and rebroadcasts the raw `Message` for widgets. The Api window (View menu) can construct and send any `Message` by hand through the enum2egui derive on the whole tree.

`MessageBusPlugin` registers every service plugin; `main.rs` adds only infrastructure (Bevy defaults, egui, the bus). A new service belongs in the `MessageBusPlugin` plugin list.

Inbound broker traffic becomes `TopicEvent`. `deliver_topic_messages` routes each one only to panes whose widget id is registered for that topic.

## Widget system (`ui/widgets.rs`, `ui/rpc.rs`)

Widgets implement `Widget` (title, ui) and `MessageHandler` (receive_message, drain_messages) and own a `WidgetRpc`. The `widgets!` macro generates `UiWidgetKind` (serialized into save files), `UiWidget` (boxed runtime instances), the conversions between them, and `get_widget_id`.

`WidgetRpc` is the only way a widget touches the outside world: subscribe and publish (JSON, typed, or binary), buffered per-topic inbound messages that the widget drains and clears, file dialogs whose tags are scoped to the widget id so two panes of the same kind cannot claim each other's results with `next_file_result`, toast notifications, and confirm modals whose results come back through `take_modal_result`. `update(context)` syncs connection state each frame and re-registers subscriptions after a reconnect. The Template widget exercises the entire surface and is the reference for writing a new widget.

## Tile layout (`services/tiles/`)

egui_tiles provides the pane and tab layout. `Pane` serializes only its `UiWidgetKind`; deserialization rebuilds a fresh widget. `VisualTree` is a resource holding the tree, the `TreeBehavior` (searchable add-widget picker, close handling, shift-drag rearrange), and layout naming state. A chained set of systems handles each frame in order: `apply_theme_and_widget_context` applies the active theme and connection state, `deliver_dialog_results` distributes filesystem and modal results to panes, `sync_modification_flags` lifts edit flags into project and layout state, `draw_shell_ui` renders the menu bar, Api window, and tree, and `flush_widget_outputs` drains widget messages onto the bus and forwards widget removals for subscription cleanup.

## Projects across processes (`services/shell.rs`)

A project file stores one tree per window. Because windows are processes, the shell coordinates over `ShellContract` topics:

- `shell/window/announce`: a child reports its window id after connecting
- `shell/window/assign-{window}`: the primary sends a layout (name plus serialized tree) to one window
- `shell/window/close-{window}`: the primary closes one window
- `shell/window/request-trees`: the primary asks every window for its current tree
- `shell/window/report-tree`: a child responds with its serialized tree
- `shell/window/request-spawn`: any window asks the primary to spawn another

Loading a project on the primary closes existing children, applies the first tree locally, and spawns one child per remaining tree; as each child announces, it is assigned the next pending layout. Saving requests trees from all known windows and collects responses until every window has reported or a one second timer expires, then writes the file directly or through a save dialog. A window that died simply drops out of the save and is pruned from the registry, so later saves do not wait on it.

Layout save and load stay local to each window, so a child can manage its own layout file without involving the primary.

## Services

- `filesystem.rs`: rfd dialogs on Bevy's async compute pool, results tagged and routed over the bus
- `modal.rs`: confirm dialogs on egui's built-in `Modal`, results delivered as bus messages keyed by id
- `notification.rs`: egui-toast toasts anchored bottom-right
- `settings.rs`: `settings.json` under the platform config directory (startup project, recent projects, theme)
- `theme.rs`: color palette presets (Deep Ocean by default, plus Dracula, Nord, Gruvbox Dark, Monokai, One Dark, Tokyo Night, Catppuccin Mocha, Solarized Dark, Sandstone, Solarized Light) built into full egui visuals and applied to the context each frame; hovering a preset in the View menu previews it live
- `fps.rs`: frame counter surfaced in the menu bar
