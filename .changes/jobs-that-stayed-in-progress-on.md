---
type: fixed
---
Jobs that stayed in progress on GitHub after their runner container stopped ([#12591](https://github.com/60db-ai/tinyhelm/issues/12591))

- TinyHelm cancels the workflow run of the job and retries the removal of the runner. New GitHub Apps get the repository permission `Actions`: write; give it to an existing App so that TinyHelm can cancel the run
