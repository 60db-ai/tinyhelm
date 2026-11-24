---
type: fixed
---
Missed the custom container labels on preview deployments, for example a `logging` label ([#2806](https://github.com/60db-ai/tinyhelm/issues/2806))

- Preview containers get the custom labels that are not Traefik, Caddy, or `tinyhelm.` labels. To protect previews with a password, use the `HTTP Basic Authentication` setting
