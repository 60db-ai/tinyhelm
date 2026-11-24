---
type: fixed
---
Missing end time and duration of deployments that timed out, failed after a worker stop, or were cancelled ([#6161](https://github.com/coollabsio/tinyhelm/issues/6161))

- TinyHelm records the end time when a deployment gets a final status, and fills it for older deployments from their last update
