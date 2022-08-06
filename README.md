![Squads](https://github.com/squads-dapp/program/blob/main/logo.png)
# squads-program
Squads V2 on-chain program

## Overview
Squads is an on-chain program that allows team to manage digital assets together, create proposals, and more. There are two types of Squads that can be created at the moment: Teams and Multisig. 

## Teams
Teams is a type of Squad, in which members are minted tokens allocated by the creator of the team. The minted tokens are locked into a PDA assigned to a member. When voting on a proposal, a user's vote weight is determined by the amount of tokens they have in their respective PDA account. The tokens are illiquid (for now), and members can vote to mint more tokens to a Squad member if they choose to. Teams allows for complex voting dynamics within a Squad.

## Multisig
In a Multisig Squad, members are not minted tokens, and the functionality centers around controlling the Vault. Members can add or remove public keys that are able to control the Vault, and members can also vote to adjust the signing threshold.

## Squad Accounts
Squads have an address (PDA), which is seeded by the public key which created it and a random string, both saved to the Squads state account to be used for derivation when needed for signing. The squad vault is also a PDA which is seeded by the Squad PDA and the string "!squadsol". The vault PDA can be used for SOL, or as a seed to derive an ATA for other tokens. Any proposal which acts to withdraw SOL or other tokens from the vault must by signed by the vault PDA (referred to as 
sol_account in the Squad state struct.

## Creating a Proposal
The required instruction data includes a byte corresponding to the type of proposal. The instruction will then fire off a followup instruction to initialize the proposal account accordingly. If a proposal passes and is executed, the execute instruction will reference the relevant fields of the proposal account according to the type of proposal specified: Withdraw, Add/Remove Member, Update Squad settings, etc. The proposal account will be created, and referenced via a PDA that is seeded by the Squad PDA and a nonce that is incremented every time a proposal is created (proposal_nonce). 

The proposal requires a fixed set of bytes corresponding to the labels (possible options to vote for - up to 5), optional description & link, etc. Inspect each Init function for the expected bytes to be unpacked and used for a proposal.

## Casting Vote
To cast a vote, the instruction is invoked with the option index of the vote choice. After creating a VoteReceipt account (derived from a PDA of the voters public key & the proposal PDA, a followup instruction is called and the vote choice index is saved to the VoteReceipt account along with a timestamp. The vote weight is then written to the Proposal account. If quorum and support are met, the proposal is marked as execute_ready and a "snapshot" of the Squads current support & supply is saved to the proposal in order to accurately reflect the state of the Squad's settings at that moment in time, in order to prevent re-opening a proposal if more members are added or if Squad settings are changed after a vote has passed or is rejected.

## Executing a Proposal
If a proposal has passed, any valid member of the Squad may then invoke the execute instruction. Depending on the type of proposal, the execute instruction requires various PDAs to be passed in ie. sol_account PDA will be required if the execution is to withdraw SOL/Token.

## Instructions
* CreateSquad
* CreateMultisig
* AddMembersToSquad
* CreateProposalAccount
* CastVote
* CastMultisigVote
* ExecuteProposal
* ExecuteMultisigProposal

## State
* Squad
* Proposal
* VoteReceipt

## Create Squad Instruction
The CreateSquad instruction requires the following serialized data, with the leading byte indictating a 0.
Be sure to look at instruction.rs to see the accounts required.
```
allocation_type: 1 byte, with value set to 1
vote_support: 1 byte, with value set between 1-100
vote_quorum: 1 byte, with value set between 1-100
core_threshold: 1 byte (not used yet), can be zero
squad_name: 24 bytes, including empty chars if shorter
description: 36 bytes, including empty chars if shorter
token: 6 bytes (not used yet), can be all empty chars
random_id: 10 bytes, should be 10 random ASCII chars as bytes
```

## Create Multisig Instruction
The CreateMultisig instruction requires the following serialized data, with the leading byte indictating a 1.
Be sure to look at instruction.rs to see the accounts required.
```
vote_quorum: 1 byte, indicating threshold, > 1 and should not exceed members_num
squad_name: 24 bytes
description: 26 bytes
random_id: 10 bytes
members_num: value of initial owner keys added
```

## Security and Liability
This software is WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.

## License
This software is released under LGPL-3.0

Software has been audited by Neodyme (report to be made available shortly).
