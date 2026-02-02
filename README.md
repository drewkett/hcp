`hcp` (short for healthcheck ping) is a simple utility that wraps terminal commands, sending a start and end
notification to healthchecks.io. It will report failures if the command exits
with a non-zero return. It will also include the output from stdout and stderr
from the command to healthchecks

```
hcp [--hcp-id HCP_ID] [--hcp-tee] [--hcp-ignore-code] [cmd [args...]]

    HCP_ID can be set using an environment variable
    --hcp-id HCP_ID   Sets the healthchecks id. This can also be set using the
                      environment variable HCP_ID
    --hcp-ignore-code Ignore the return code from cmd. Also available using HCP_IGNORE_CODE
    --hcp-tee         Controls whether to also output the cmd stdout/stderr to the local
                      stdout/stderr. By default the output from the cmd will only get
                      passed as text to healthchecks. This option can also be enabled
                      using the environment variable HCP_TEE. Only the existance of the
                      variable is checked
    [cmd [args...]]   If no command is passed, the healthcheck will be notified as a
                      success with the text 'No command given'
```

# Install instructions

This utility is written in Rust. The normal cargo install procedure works

```
cargo install hcp
```

# Environment variables

The following environment variables are stripped from the child process so they
are not leaked to the wrapped command:

- `HCP_ID`
- `HCP_TEE`
- `HCP_IGNORE_CODE`

# Exit codes

When hcp itself encounters an error it exits with one of these codes:

| Code | Meaning |
|------|---------|
| 961  | Failed to spawn the child process |
| 962  | I/O error reading child output |
| 963  | Healthcheck HTTP request failed |
| 964  | Child exited without an exit code (e.g. killed by signal) |

If the child process exits normally, hcp forwards its exit code (unless
`--hcp-ignore-code` is set, in which case it exits 0).

hcp will exit immediately if the healthcheck HTTP request to `/start` fails.
HTTP requests are retried once after a 2-second delay on 5xx or connection
errors.

On unix, SIGTERM and SIGINT are forwarded to the child process.
