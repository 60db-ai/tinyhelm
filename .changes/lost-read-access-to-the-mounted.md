---
type: fixed
---
Lost read access to the mounted `tinyhelm-ca.crt` file in non-root containers after a database start or a CA certificate change ([#6081](https://github.com/60db-ai/tinyhelm/issues/6081))

- TinyHelm now writes the CA file in place, so running containers keep their mount and see the new certificate
