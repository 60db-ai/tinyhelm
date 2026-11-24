---
type: fixed
---
Left the containers, volumes, and files of the resources on the server after `Delete server` with `Delete all resources`, and could leave resources without a server in TinyHelm ([#12551](https://github.com/coollabsio/tinyhelm/issues/12551))

- TinyHelm deletes the resources one after the other before it deletes the server, and keeps the server until all of them are deleted
- When the delete does not finish within a day, TinyHelm deletes the resources and the server from TinyHelm and leaves the containers on the server
