---
type: fixed
---
Failed Docker Compose parsing and deployments for domains saved with `ws://` or `wss://` by older versions ([#6003](https://github.com/60db-ai/tinyhelm/issues/6003))

- The upgrade rewrites these domains to `http://` and `https://`. The proxy routes WebSocket connections over these domains
