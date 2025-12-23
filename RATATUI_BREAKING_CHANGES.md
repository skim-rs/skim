## Binds

- `execute(...)` will still run the command if no item is selected. To get the previous behavior back, use `if-non-matched()+execute(...)`
- field expansion in `execute` and `preview` will no longer support arbitrary spaces, for instance  `{ }` will not get expanded as `{}`
- interactive mode will not use stdin/skim default command when starting up.
- `expect` bind is deprecated
- interactive mode will now expand like other commands (`{}` for the currently selected item, `{q}` for the query...)