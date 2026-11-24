---
type: fixed
---
Failed deployments and server commands when sshd closed a new SSH connection before the key exchange (`kex_exchange_identification`) ([#6422](https://github.com/60db-ai/tinyhelm/issues/6422))

- TinyHelm tries such a connection up to 5 times in about 30 seconds. Set `SSH_HANDSHAKE_MAX_RETRIES` to change the number of attempts
