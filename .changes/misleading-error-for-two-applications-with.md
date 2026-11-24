---
type: fixed
---
Misleading error for two applications with the same `Docker Image` and different `Docker Image Tag` values ([#12568](https://github.com/coollabsio/tinyhelm/issues/12568))

- The error and the `Docker Image Tag` help tell that TinyHelm always pushes the commit SHA tag, so each application needs its own `Docker Image`
