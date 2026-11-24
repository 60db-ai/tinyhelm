---
type: fixed
---
Ignored PostgreSQL init scripts, the default MongoDB database, and file mounts on servers with a strict umask ([#4519](https://github.com/60db-ai/tinyhelm/issues/4519))

- TinyHelm makes these files readable for the container user after each write
