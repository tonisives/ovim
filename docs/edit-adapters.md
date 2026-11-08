# Edit adapter scripts

Ovim can delegate text capture and synchronization to a script, like its custom
terminal launcher. The terminal launcher controls the editor window. The edit
adapter controls the source text field. They can be used together or separately.

Create an executable `edit-adapter` beside `settings.yaml` in Ovim's configuration
directory (`~/Library/Application Support/ovim/` on macOS). Ovim invokes it with
an operation as its first argument and a JSON request on standard input. Return
one JSON object on standard output. A nonzero exit or invalid response causes
Ovim to paste the saved file on exit. Calls have a ten-second deadline.

| Operation | Request | Response |
| --- | --- | --- |
| `capture` | `{"bundle_id":"application.bundle"}` | `{"handled":false}` to fall through, or the capture object below |
| `update` | `{"context":{},"text":"new text","token":null}` | `{"token":"editor-id"}` after the editor accepts the update |
| `finish` | `{"context":{},"token":"editor-id"}` | `{}` after restoring temporary selection styling |
| `cursor` | `{"context":{},"line":0,"column":3}` | `{}` after restoring the cursor |

A handled capture returns:

```json
{
  "handled": true,
  "found": true,
  "text": "existing draft",
  "cursor": { "line": 0, "column": 3 },
  "frame": { "x": 100, "y": 200, "width": 500, "height": 100 },
  "hostname": "example.com",
  "context": { "target": "opaque application target" }
}
```

`cursor`, `frame`, and `hostname` may be null or omitted. Frame coordinates are
macOS screen points. Ovim passes `context` unchanged to subsequent operations;
the adapter should use it to pin the original target throughout the session.
`found: false` falls back to normal text capture and paste. Native accessibility
live sync remains available when the adapter returns `handled: false`.

`update` receives the complete buffer. Its token is null on the first call and
then contains the last successful update's token. `finish` runs after the final
queued update, before Ovim returns focus. `cursor` runs after focus restoration.
Keep page operations in the background so they do not activate browser windows.
Adapter scripts run with your user permissions.

## bmux adapter

The optional adapter in `adapters/bmux/` owns pane discovery and trusted text
input. It supports native inputs, textareas, contenteditable editors, Draft.js,
and nested shadow editors. Ovim's Rust editor code only knows the adapter contract.

Install it with:

```sh
node scripts/install-edit-adapter.mjs adapters/bmux/adapter.mjs
```

The installer copies the adapter and shared browser scripts into Ovim's config
directory and writes a launcher using the current Node executable. It preserves
an existing custom dispatcher. Re-run the command to update an installed adapter.
Remove `edit-adapter` to disable it. No app restart is needed for adapter changes.

bmux browser commands accept callers without an agent identity by default. If
you previously enabled identity requirements, disable **Require agent ID** and
**Require agent type** in bmux's Bot settings. The adapter edits on your behalf
and supplies no agent identity.
