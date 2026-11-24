---
type: fixed
---
Failed image scan of images pinned by tag and digest (`name:tag@sha256:<digest>`), which blocked deployments when `If the scan cannot run` blocks the deploy ([#12564](https://github.com/60db-ai/tinyhelm/issues/12564))

- The scan reads such an image by its digest, as Docker does
