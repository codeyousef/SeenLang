# Environment

Module: `env/env`

Environment helpers read process environment variables and expose small runtime
configuration utilities.

```seen
import env::env
```

Use this module when application behavior needs to inspect the host environment
without directly calling runtime C helpers.
