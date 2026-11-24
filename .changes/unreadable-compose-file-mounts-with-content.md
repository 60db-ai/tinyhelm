---
type: fixed
---
Unreadable Compose file mounts with `content:` for a non-root container user on servers with a strict umask ([#4490](https://github.com/60db-ai/tinyhelm/issues/4490))

- TinyHelm makes the file readable for every user when it writes the file on the first start
