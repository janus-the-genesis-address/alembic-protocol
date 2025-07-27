---
title: Alembic Test Validator
pagination_label: "Alembic CLI: Test Validator"
sidebar_label: Test Validator
---

During early stage development, it is often convenient to target a cluster with
fewer restrictions and more configuration options than the public offerings
provide. This is easily achieved with the `Alembic-test-validator` binary, which
starts a full-featured, single-node cluster on the developer's workstation.

## Advantages

- No RPC rate-limits
- No airdrop limits
- Direct [on-chain program](https://genesisaddress.ai/docs/programs) deployment
  (`--bpf-program ...`)
- Clone accounts from a public cluster, including programs (`--clone ...`)
- Load accounts from files
- Configurable transaction history retention (`--limit-ledger-size ...`)
- Configurable epoch length (`--slots-per-epoch ...`)
- Jump to an arbitrary slot (`--warp-slot ...`)

## Installation

The `Alembic-test-validator` binary ships with the Alembic CLI Tool Suite.
[Install](../install.md) before continuing.

## Running

First take a look at the configuration options

```
Alembic-test-validator --help
```

Next start the test validator

```
Alembic-test-validator
```

By default, basic status information is printed while the process is running.
See [Appendix I](#appendix-i-status-output) for details

```
Ledger location: test-ledger
Log: test-ledger/validator.log
Identity: EPhgPANa5Rh2wa4V2jxt7YbtWa3Uyw4sTeZ13cQjDDB8
Genesis Hash: 4754oPEMhAKy14CZc8GzQUP93CB4ouELyaTs4P8ittYn
Version: 1.6.7
Shred Version: 13286
Gossip Address: 127.0.0.1:1024
TPU Address: 127.0.0.1:1027
JSON RPC URL: http://127.0.0.1:8899
⠈ 00:36:02 | Processed Slot: 5142 | Confirmed Slot: 5142 | Finalized Slot: 5110 | Snapshot Slot: 5100 | Transactions: 5142 | ◎499.974295000
```

Leave `Alembic-test-validator` running in its own terminal. When it is no longer
needed, it can be stopped with ctrl-c.

## Interacting

Open a new terminal to interact with a [running](#running) `Alembic-test-validator`
instance using other binaries from the Alembic CLI Tool Suite or your own client
software.

#### Configure the CLI Tool Suite to target a local cluster by default

```
Alembic config set --url http://127.0.0.1:8899
```

#### Verify the CLI Tool Suite configuration

```
Alembic genesis-hash
```

- **NOTE:** The result should match the `Genesis Hash:` field in the
  `Alembic-test-validator` status output

#### Check the wallet balance

```
Alembic balance
```

- **NOTE:** `Error: No such file or directory (os error 2)` means that the default
  wallet does not yet exist. Create it with `Alembic-keygen new`.
- **NOTE:** If the wallet has a zero TACHYON balance, airdrop some localnet TACHYON with
  `Alembic airdrop 10`

#### Perform a basic transfer transaction

```
Alembic transfer EPhgPANa5Rh2wa4V2jxt7YbtWa3Uyw4sTeZ13cQjDDB8 1
```

#### Monitor `msg!()` output from on-chain programs

```
Alembic logs
```

- **NOTE:** This command needs to be running when the target transaction is
  executed. Run it in its own terminal

## Appendix I: Status Output

```
Ledger location: test-ledger
```

- File path of the ledger storage directory. This directory can get large. Store
  less transaction history with `--limit-ledger-size ...` or relocate it with
  `--ledger ...`

```
Log: test-ledger/validator.log
```

- File path of the validator text log file. The log can also be streamed by
  passing `--log`. Status output is suppressed in this case.

```
Identity: EPhgPANa5Rh2wa4V2jxt7YbtWa3Uyw4sTeZ13cQjDDB8
```

- The validator's identity in the [gossip network](../../validator/gossip.md#gossip-overview)

```
Version: 1.6.7
```

- The software version

```
Gossip Address: 127.0.0.1:1024
TPU Address: 127.0.0.1:1027
JSON RPC URL: http://127.0.0.1:8899
```

- The network address of the [Gossip](../../validator/gossip.md#gossip-overview),
  [Transaction Processing Unit](../../validator/tpu.md) and [JSON RPC](https://genesisaddress.ai/docs/rpc)
  service, respectively

```
⠈ 00:36:02 | Processed Slot: 5142 | Confirmed Slot: 5142 | Finalized Slot: 5110 | Snapshot Slot: 5100 | Transactions: 5142 | ◎499.974295000
```

- Session running time, current slot of the three block
  [commitment levels](https://genesisaddress.ai/docs/rpc#configuring-state-commitment),
  slot height of the last snapshot, transaction count,
  [voting authority](../../operations/guides/vote-accounts.md#vote-authority) balance

## Appendix II: Runtime Features

By default, the test validator runs with all [runtime features](https://genesisaddress.ai/docs/core/runtime#features) activated.

You can verify this using the [Alembic command-line tools](../install.md):

```bash
Alembic feature status -ul
```

Since this may not always be desired, especially when testing programs meant for deployment to mainnet, the CLI provides an option to deactivate specific features:

```bash
Alembic-test-validator --deactivate-feature <FEATURE_PUBKEY_1> --deactivate-feature <FEATURE_PUBKEY_2>
```
