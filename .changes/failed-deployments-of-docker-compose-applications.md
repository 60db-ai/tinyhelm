---
type: fixed
---
Failed deployments of Docker Compose applications that had an empty copy of an environment variable ([#6295](https://github.com/60db-ai/tinyhelm/issues/6295))

- The upgrade removes extra copies of a variable that older versions added, and keeps the most recently changed copy that has a value
