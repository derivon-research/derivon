# Runtime Contract

Derivon graph commands are offline, stateless mathematical operations.

They do not:

- access the network;
- send telemetry, check for updates, or report crashes;
- read user-level configuration;
- create caches, state directories, or lock files;
- search the current or parent directories for implicit graph input; or
- vary JSON fields, help, or diagnostics with the system locale.

The CLI reads only stdin and files explicitly named by command arguments. The current
working directory is used only to resolve relative paths. HOME, locale, and network state
do not alter the result for the same explicit input.

Machine fields, help, and diagnostic messages are English. Error messages are not stable
for parsing; machine decisions use error codes and paths. The user manual is bilingual,
with English as the normative version.
