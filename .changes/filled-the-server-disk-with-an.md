---
type: fixed
---
Filled the server disk with an unbounded TinyHelm `laravel.log` file ([#3578](https://github.com/coollabsio/tinyhelm/issues/3578))

- TinyHelm writes its log to one file for each day and keeps the files of the last 7 days. Set `LOG_DAILY_DAYS` in `/data/tinyhelm/source/.env` to keep more or fewer days
- At the start, TinyHelm compresses an old `laravel.log` once to `laravel-legacy-<date>.log.gz` and deletes it
