# Chain-scoped database

**Version:** 1.0.2

## Problem

The message database was stored per wallet but not per chain:

```
~/.config/taolk/<wallet>/messages.db
```

Switching between mainnet and testnet with the same wallet name mixed messages from both chains into one database.

## Fix

The database path now includes a chain identifier derived from the genesis hash:

```
~/.config/taolk/<wallet>/<genesis_hash_prefix>/messages.db
```

The prefix is the first 8 hex characters (4 bytes) of the chain's genesis hash, which uniquely identifies the chain.

## Migration

Automatic. On first launch after updating:

1. If a legacy `messages.db` exists at the wallet root, it is moved into the subdirectory for the chain you connect to.
2. No data is lost. The existing database becomes the database for that chain.

If you previously used the same wallet on multiple chains, the migrated database will contain messages from all chains. There is no way to automatically separate them. You can delete the database and re-sync from the mirror if needed.
