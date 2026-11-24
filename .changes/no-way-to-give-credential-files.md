---
type: fixed
---
No way to give credential files, such as a Google Cloud service account key, to a `Custom Fluent Bit` log drain ([#7163](https://github.com/60db-ai/tinyhelm/issues/7163))

- Files in `/data/tinyhelm/log-drains/files` on the server are mounted read-only at `/files` in the log drain container, for example `google_service_credentials /files/gcp.json`
