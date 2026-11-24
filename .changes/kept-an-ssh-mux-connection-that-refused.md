---
type: fixed
---
Kept an SSH mux connection that refused new sessions, so each command opened a new SSH connection and logs showed `Session open refused by peer` until the mux files were deleted by hand ([#4173](https://github.com/coollabsio/tinyhelm/issues/4173))

- TinyHelm replaces the mux connection when the server refuses a new session on it
