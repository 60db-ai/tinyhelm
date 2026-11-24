---
type: fixed
---
Refused connections to Supabase PostgreSQL databases with `Enable SSL` on, because the database listened only on localhost and lost its preloaded extensions ([#5574](https://github.com/60db-ai/tinyhelm/issues/5574))

- TinyHelm now keeps the Supabase config directory when it starts the database with SSL
