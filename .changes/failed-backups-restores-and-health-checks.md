---
type: fixed
---
Failed backups, restores, and health checks of Supabase PostgreSQL databases with `Peer authentication failed for user` when the database user was not `postgres` or `supabase_admin` ([#5325](https://github.com/60db-ai/tinyhelm/issues/5325))

- TinyHelm connects to databases with a Supabase image over 127.0.0.1 instead of the socket
