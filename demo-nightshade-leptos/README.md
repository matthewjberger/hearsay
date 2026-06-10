# demo-nightshade-leptos

A hearsay demo built on the Nightshade Leptos/webview architecture, with feature parity with the bevy and retained-UI demos: multi-window process spawning, template panels with publish/subscribe, toast notifications, confirm modals, file actions, project and layout save/load with cross-window collection, the eleven themes, and an Api composer. The whole engine runs inside a web worker against an OffscreenCanvas and renders through WebGPU off the main thread. A [Leptos](https://leptos.dev) UI drives it from the main thread, and a native webview shell turns the same bundle into a desktop app that also hosts the hearsay broker.

The New Window button publishes on a shell topic; the broker-hosting shell answers by spawning another copy of itself under hearsay's process supervision, and the new window's page joins the same broker, announces itself, and receives its layout assignment. Closing a project closes the spawned windows through their close topics, and saving a project collects every window's layout over the broker before writing the file, exactly like the native demos.

The page itself is a first-class hearsay peer: it opens a WebSocket session against the broker's websocket listener and speaks the hearsay wire format directly, postcard-encoded `PeerEvent` frames out and `Message` frames in. It publishes and subscribes on the same `template/text` and `template/binary` topics as the native demos, so messages flow between this app and `demo-nightshade` windows through one broker. Publishing on `template/spawn` spawns a cube in the engine viewport of every connected Leptos window.

## Workspace

- `protocol`, the message and data types the page and worker share, plus the postMessage envelope keys.
- `worker`, the wasm module inside the web worker. The engine `World` plus a `TemplateWorld` (its own `freecs` world) driven by system functions in `worker/src/systems/`.
- the root crate (`page`), the Leptos UI: the viewport, the renderer HUD, and the hearsay panel with the websocket peer in `src/hearsay_link.rs`.
- `desktop`, the native shell: a webview window over the web bundle, served from an ephemeral localhost port, plus the hearsay broker on `127.0.0.1:9612` with its websocket listener on `127.0.0.1:9613`. When another broker already owns the port (a running `demo-nightshade` primary, which opens the same websocket listener), the shell skips hosting and the page connects to that broker instead.

## Quickstart

Tooling is pinned in [mise.toml](mise.toml). Install [mise](https://mise.jdx.dev) and [just](https://github.com/casey/just), then:

```bash
just init
just run
```

`just run` builds the worker, builds the bundle with Trunk, and opens the app in a native webview window. `just run-web` serves the same bundle at http://127.0.0.1:8080 instead; in that mode start a broker separately (run `demo-nightshade` or the desktop shell). The browser path needs WebGPU and OffscreenCanvas-in-workers support (Chromium 113+, Firefox 141+). The worker compiles the whole engine, so the first build is large.

## Trying the interop

1. `just run-demo-nightshade` from the repo root first (its primary window hosts the broker and the websocket listener), then `just run` here. The order matters: the native demo always hosts the broker itself, while this shell skips hosting when the port is taken.
2. Add a Template panel in the native window and publish a message: it appears in the Leptos page's received log, and the page's publishes appear in the native panel.
3. Click "Spawn Cube via Broker" in the page (or publish anything to `template/spawn`): every connected Leptos window spawns a cube in its viewport.

Running this app alone works too: the desktop shell hosts the broker itself.

## License

Dual-licensed under MIT or Apache-2.0, at your option.
