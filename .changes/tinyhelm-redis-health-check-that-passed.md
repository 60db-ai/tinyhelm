---
type: fixed
---
TinyHelm Redis health check that passed on any reply, also when Redis rejected the password or was still loading its data

- The health checks of the TinyHelm Redis and PostgreSQL containers run without a shell, and the Redis health check pings with the Redis password
