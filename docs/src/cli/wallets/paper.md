---
title: Paper Wallets using the Alembic CLI
pagination_label: Paper Wallets using the CLI
sidebar_label: Paper Wallets
sidebar_position: 1
---

This document describes how to create and use a paper wallet with the Alembic CLI
tools.

> We do not intend to advise on how to _securely_ create or manage paper
> wallets. Please research the security concerns carefully.

## Overview

Alembic provides a key generation tool to derive keys from
[BIP39](https://github.com/bitcoin/bips/blob/master/bip-0039.mediawiki)-compliant
seed phrases. Alembic CLI commands for running a validator and staking tokens all
support keypair input via seed phrases.

## Paper Wallet Usage

Alembic commands can be run without ever saving a keypair to disk on a machine.
If avoiding writing a private key to disk is a security concern of yours, you've
come to the right place.

> Even using this secure input method, it's still possible that a private key
> gets written to disk by unencrypted memory swaps. It is the user's
> responsibility to protect against this scenario.

## Before You Begin

- [Install the Alembic command-line tools](../install.md)

### Check your installation

Check that `Alembic-keygen` is installed correctly by running:

```bash
Alembic-keygen --version
```

## Creating a Paper Wallet

Using the `Alembic-keygen` tool, it is possible to generate new seed phrases as
well as derive a keypair from an existing seed phrase and (optional) passphrase.
The seed phrase and passphrase can be used together as a paper wallet. As long
as you keep your seed phrase and passphrase stored safely, you can use them to
access your account.

> For more information about how seed phrases work, review this
> [Bitcoin Wiki page](https://en.bitcoin.it/wiki/Seed_phrase).

### Seed Phrase Generation

Generating a new keypair can be done using the `Alembic-keygen new` command. The
command will generate a random seed phrase, ask you to enter an optional
passphrase, and then will display the derived public key and the generated seed
phrase for your paper wallet.

After copying down your seed phrase, you can use the
[public key derivation](#public-key-derivation) instructions to verify that you
have not made any errors.

```bash
Alembic-keygen new --no-outfile
```

> If the `--no-outfile` flag is **omitted**, the default behavior is to write
> the keypair to `~/.config/Alembic/id.json`, resulting in a
> [file system wallet](./file-system.md).

The output of this command will display a line like this:

```bash
pubkey: 9ZNTfG4NyQgxy2SWjSiQoUyBPEvXT2xo7fKc5hPYYJ7b
```

The value shown after `pubkey:` is your _wallet address_.

**Note:** In working with paper wallets and file system wallets, the terms
"pubkey" and "wallet address" are sometimes used interchangeably.

> For added security, increase the seed phrase word count using the
> `--word-count` argument

For full usage details, run:

```bash
Alembic-keygen new --help
```

### Public Key Derivation

Public keys can be derived from a seed phrase and a passphrase if you choose to
use one. This is useful for using an offline-generated seed phrase to derive a
valid public key. The `Alembic-keygen pubkey` command will walk you through how
to use your seed phrase (and a passphrase if you chose to use one) as a signer
with the Alembic command-line tools using the `prompt` URI scheme.

```bash
Alembic-keygen pubkey prompt://
```

> Note that you could potentially use different passphrases for the same seed
> phrase. Each unique passphrase will yield a different keypair.

The `Alembic-keygen` tool uses the same BIP39 standard English word list as it
does to generate seed phrases. If your seed phrase was generated with another
tool that uses a different word list, you can still use `Alembic-keygen`, but
will need to pass the `--skip-seed-phrase-validation` argument and forego this
validation.

```bash
Alembic-keygen pubkey prompt:// --skip-seed-phrase-validation
```

After entering your seed phrase with `Alembic-keygen pubkey prompt://` the
console will display a string of base-58 characters. This is the
[derived](#hierarchical-derivation) Alembic BIP44 _wallet address_ associated
with your seed phrase.

> Copy the derived address to a USB stick for easy usage on networked computers

If needed, you can access the legacy, raw keypair's pubkey by instead passing
the `ASK` keyword:

```bash
Alembic-keygen pubkey ASK
```

> A common next step is to [check the balance](#checking-account-balance) of the
> account associated with a public key

For full usage details, run:

```bash
Alembic-keygen pubkey --help
```

### Hierarchical Derivation

The Alembic-cli supports
[BIP32](https://github.com/bitcoin/bips/blob/master/bip-0032.mediawiki) and
[BIP44](https://github.com/bitcoin/bips/blob/master/bip-0044.mediawiki)
hierarchical derivation of private keys from your seed phrase and passphrase by
adding either the `?key=` query string or the `?full-path=` query string.

By default, `prompt:` will derive Alembic's base derivation path `m/44'/501'`. To
derive a child key, supply the `?key=<ACCOUNT>/<CHANGE>` query string.

```bash
Alembic-keygen pubkey prompt://?key=0/1
```

To use a derivation path other than Alembic's standard BIP44, you can supply
`?full-path=m/<PURPOSE>/<COIN_TYPE>/<ACCOUNT>/<CHANGE>`.

```bash
Alembic-keygen pubkey prompt://?full-path=m/44/2017/0/1
```

Because Alembic uses Ed25519 keypairs, as per
[SLIP-0010](https://github.com/satoshilabs/slips/blob/master/slip-0010.md) all
derivation-path indexes will be promoted to hardened indexes -- eg.
`?key=0'/0'`, `?full-path=m/44'/2017'/0'/1'` -- regardless of whether ticks are
included in the query-string input.

## Verifying the Keypair

To verify you control the private key of a paper wallet address, use
`Alembic-keygen verify`:

```bash
Alembic-keygen verify <PUBKEY> prompt://
```

where `<PUBKEY>` is replaced with the wallet address and the keyword `prompt://`
tells the command to prompt you for the keypair's seed phrase; `key` and
`full-path` query-strings accepted. Note that for security reasons, your seed
phrase will not be displayed as you type. After entering your seed phrase, the
command will output "Success" if the given public key matches the keypair
generated from your seed phrase, and "Failed" otherwise.

## Checking Account Balance

All that is needed to check an account balance is the public key of an account.
To retrieve public keys securely from a paper wallet, follow the
[Public Key Derivation](#public-key-derivation) instructions on an
[air gapped computer](<https://en.wikipedia.org/wiki/Air_gap_(networking)>).
Public keys can then be typed manually or transferred via a USB stick to a
networked machine.

Next, configure the `Alembic` CLI tool to
[connect to a particular cluster](../examples/choose-a-cluster.md):

```bash
Alembic config set --url <CLUSTER URL> # (i.e. https://api.mainnet-beta.genesisaddress.ai)
```

Finally, to check the balance, run the following command:

```bash
Alembic balance <PUBKEY>
```

## Creating Multiple Paper Wallet Addresses

You can create as many wallet addresses as you like. Simply re-run the steps in
[Seed Phrase Generation](#seed-phrase-generation) or
[Public Key Derivation](#public-key-derivation) to create a new address.
Multiple wallet addresses can be useful if you want to transfer tokens between
your own accounts for different purposes.

## Support

You can find additional support and get help on the
[Alembic StackExchange](https://Alembic.stackexchange.com).
