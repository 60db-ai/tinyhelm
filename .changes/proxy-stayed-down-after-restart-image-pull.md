---
type: fixed
---
Proxy that stopped and stayed down after a `Restart Proxy` when its image could not be pulled ([#12567](https://github.com/60db-ai/tinyhelm/issues/12567))

- TinyHelm pulls the proxy image before it stops the proxy. If the pull fails, the restart fails and the proxy keeps running
