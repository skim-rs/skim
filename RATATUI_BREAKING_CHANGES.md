## Binds

- `execute(...)` will still run the command if no item is selected. To get the previous behavior back, use `if-non-matched()+execute(...)`
- field expansion in `execute` and `preview` will no longer support arbitrary spaces, for instance  `{ }` will not get expanded as `{}`