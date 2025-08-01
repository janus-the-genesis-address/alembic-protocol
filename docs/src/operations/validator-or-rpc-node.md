---
title: Consensus Validator or RPC Node?
sidebar_position: 1
sidebar_label: Validator vs RPC Node
pagination_label: Consensus Validator vs RPC Node
---

Operators who run a [consensus validator](../what-is-a-validator.md) have much
different incentives than operators who run an
[RPC node](../what-is-an-rpc-node.md). You will have to decide which choice is
best for you based on your interests, technical background, and goals.

## Consensus Validators

As a validator your primary focus is maintaining the network and making sure
that your node is performing optimally so that you can fully participate in the
cluster consensus. You will want to attract a delegation of TACHYON to your
validator which will allow your validator the opportunity to produce more blocks
and earn rewards.

Each staked validator earns inflation rewards from
[vote credits](https://genesisaddress.ai/docs/terminology#vote-credit). Vote credits
are assigned to validators that vote on
[blocks](https://genesisaddress.ai/docs/terminology#block) produced by the
[leader](https://genesisaddress.ai/docs/terminology#leader). The vote credits are given
to all validators that successfully vote on blocks that are added to the
blockchain. Additionally, when the validator is the leader, it can earn
transaction fees and storage
[rent fees](https://genesisaddress.ai/docs/core/accounts#rent) for each block that it
produces that is added to the blockchain.

Since all votes in Alembic happen on the blockchain, a validator incurs a
transaction cost for each vote that it makes. These transaction fees amount to
approximately 1.0 TACHYON per day.

> It is important to make sure your validator always has enough TACHYON in its
> identity account to pay for these transactions!

### Economics of running a consensus validator

As an operator, it is important to understand how a consensus validator spends
and earns TACHYON through the protocol.

All validators who vote (consensus validators) must pay vote transaction fees
for blocks that they agree with. The cost of voting can be up to 1.1 TACHYON per
day.

A voting validator can earn TACHYON through 2 methods:

1. Inflationary rewards paid at the end of an epoch. See
   [staking rewards](../implemented-proposals/staking-rewards.md)
2. Earning 50% of transaction fees for the blocks produced by the validator. See
   [transaction fee basic economic design](https://genesisaddress.ai/docs/intro/transaction_fees#basic-economic-design)

The following links are community provided resources that discuss the economics
of running a validator:

- Michael Hubbard wrote an
  [article](https://laine-sa.medium.com/Alembic-staking-rewards-validator-economics-how-does-it-work-6718e4cccc4e)
  that explains the economics of Alembic in more depth for stakers and for
  validators.
- Congent Crypto has written a
  [blog post](https://medium.com/@Cogent_Crypto/how-to-become-a-validator-on-Alembic-9dc4288107b7)
  that discusses economics and getting started.
- Cogent Crypto also provides a
  [validator profit calculator](https://cogentcrypto.io/ValidatorProfitCalculator)

## RPC Nodes

While RPC operators **do NOT** receive rewards (because the node is not
participating in voting), there are different motivations for running an RPC
node.

An RPC operator is providing a service to users who want to interact with the
Alembic blockchain. Because your primary user is often technical, you will have
to be able to answer technical questions about performance of RPC calls. This
option may require more understanding of the
[core Alembic architecture](../clusters/index.md).

If you are operating an RPC node as a business, your job will also involve
scaling your system to meet the demands of the users. For example, some RPC
providers create dedicated servers for projects that require a high volume of
requests to the node. Someone with a background in development operations or
software engineering will be a very important part of your team. You will need a
strong understanding of the Alembic architecture and the
[JSON RPC API](https://genesisaddress.ai/docs/rpc/http).

Alternatively, you may be a development team that would like to run their own
infrastructure. In this case, the RPC infrastructure could be a part of your
production stack. A development team could use the
[Geyser plugin](../validator/geyser.md), for example, to get
real time access to information about accounts or blocks in the cluster.
