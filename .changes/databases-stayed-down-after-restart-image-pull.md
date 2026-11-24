---
type: fixed
---
Databases that stopped and stayed down after a `Restart` when the image could not be pulled ([#12567](https://github.com/60db-ai/tinyhelm/issues/12567))

- TinyHelm stops the database only after the image pull. If the pull fails, the restart fails and the database keeps running
