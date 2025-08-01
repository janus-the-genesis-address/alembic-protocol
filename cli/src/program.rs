use {
    crate::{
        checks::*,
        cli::{
            log_instruction_custom_error, CliCommand, CliCommandInfo, CliConfig, CliError,
            ProcessResult,
        },
    },
    bip39::{Language, Mnemonic, MnemonicType, Seed},
    clap::{App, AppSettings, Arg, ArgMatches, SubCommand},
    log::*,
    Alembic_account_decoder::{UiAccountEncoding, UiDataSliceConfig},
    Alembic_bpf_loader_program::syscalls::create_program_runtime_environment_v1,
    Alembic_clap_utils::{
        self,
        fee_payer::{fee_payer_arg, FEE_PAYER_ARG},
        hidden_unless_forced,
        input_parsers::*,
        input_validators::*,
        keypair::*,
        offline::{OfflineArgs, DUMP_TRANSACTION_MESSAGE, SIGN_ONLY_ARG},
    },
    Alembic_cli_output::{
        return_signers_with_config, CliProgram, CliProgramAccountType, CliProgramAuthority,
        CliProgramBuffer, CliProgramId, CliUpgradeableBuffer, CliUpgradeableBuffers,
        CliUpgradeableProgram, CliUpgradeableProgramClosed, CliUpgradeableProgramExtended,
        CliUpgradeablePrograms, ReturnSignersConfig,
    },
    Alembic_client::{
        connection_cache::ConnectionCache,
        send_and_confirm_transactions_in_parallel::{
            send_and_confirm_transactions_in_parallel_blocking, SendAndConfirmConfig,
        },
        tpu_client::{TpuClient, TpuClientConfig},
    },
    Alembic_program_runtime::{compute_budget::ComputeBudget, invoke_context::InvokeContext},
    Alembic_rbpf::{elf::Executable, verifier::RequisiteVerifier},
    Alembic_remote_wallet::remote_wallet::RemoteWalletManager,
    Alembic_rpc_client::rpc_client::RpcClient,
    Alembic_rpc_client_api::{
        client_error::ErrorKind as ClientErrorKind,
        config::{RpcAccountInfoConfig, RpcProgramAccountsConfig, RpcSendTransactionConfig},
        filter::{Memcmp, RpcFilterType},
    },
    Alembic_rpc_client_nonce_utils::blockhash_query::BlockhashQuery,
    Alembic_sdk::{
        account::Account,
        account_utils::StateMut,
        bpf_loader, bpf_loader_deprecated,
        bpf_loader_upgradeable::{self, UpgradeableLoaderState},
        feature_set::FeatureSet,
        instruction::{Instruction, InstructionError},
        loader_instruction,
        message::Message,
        native_token::TACHYON,
        packet::PACKET_DATA_SIZE,
        pubkey::Pubkey,
        signature::{keypair_from_seed, read_keypair_file, Keypair, Signature, Signer},
        system_instruction::{self, SystemError},
        system_program,
        transaction::{Transaction, TransactionError},
    },
    std::{
        fs::File,
        io::{Read, Write},
        mem::size_of,
        path::PathBuf,
        rc::Rc,
        str::FromStr,
        sync::Arc,
    },
};

pub const CLOSE_PROGRAM_WARNING: &str = "WARNING! Closed programs cannot be recreated at the same \
                                         program id. Once a program is closed, it can never be \
                                         invoked again. To proceed with closing, rerun the \
                                         `close` command with the `--bypass-warning` flag";

#[derive(Debug, PartialEq, Eq)]
pub enum ProgramCliCommand {
    Deploy {
        program_location: Option<String>,
        fee_payer_signer_index: SignerIndex,
        program_signer_index: Option<SignerIndex>,
        program_pubkey: Option<Pubkey>,
        buffer_signer_index: Option<SignerIndex>,
        buffer_pubkey: Option<Pubkey>,
        upgrade_authority_signer_index: SignerIndex,
        is_final: bool,
        max_len: Option<usize>,
        allow_excessive_balance: bool,
        skip_fee_check: bool,
    },
    Upgrade {
        fee_payer_signer_index: SignerIndex,
        program_pubkey: Pubkey,
        buffer_pubkey: Pubkey,
        upgrade_authority_signer_index: SignerIndex,
        sign_only: bool,
        dump_transaction_message: bool,
        blockhash_query: BlockhashQuery,
    },
    WriteBuffer {
        program_location: String,
        fee_payer_signer_index: SignerIndex,
        buffer_signer_index: Option<SignerIndex>,
        buffer_pubkey: Option<Pubkey>,
        buffer_authority_signer_index: SignerIndex,
        max_len: Option<usize>,
        skip_fee_check: bool,
    },
    SetBufferAuthority {
        buffer_pubkey: Pubkey,
        buffer_authority_index: Option<SignerIndex>,
        new_buffer_authority: Pubkey,
    },
    SetUpgradeAuthority {
        program_pubkey: Pubkey,
        upgrade_authority_index: Option<SignerIndex>,
        new_upgrade_authority: Option<Pubkey>,
        sign_only: bool,
        dump_transaction_message: bool,
        blockhash_query: BlockhashQuery,
    },
    SetUpgradeAuthorityChecked {
        program_pubkey: Pubkey,
        upgrade_authority_index: SignerIndex,
        new_upgrade_authority_index: SignerIndex,
        sign_only: bool,
        dump_transaction_message: bool,
        blockhash_query: BlockhashQuery,
    },
    Show {
        account_pubkey: Option<Pubkey>,
        authority_pubkey: Pubkey,
        get_programs: bool,
        get_buffers: bool,
        all: bool,
        use_lamports_unit: bool,
    },
    Dump {
        account_pubkey: Option<Pubkey>,
        output_location: String,
    },
    Close {
        account_pubkey: Option<Pubkey>,
        recipient_pubkey: Pubkey,
        authority_index: SignerIndex,
        use_lamports_unit: bool,
        bypass_warning: bool,
    },
    ExtendProgram {
        program_pubkey: Pubkey,
        additional_bytes: u32,
    },
}

pub trait ProgramSubCommands {
    fn program_subcommands(self) -> Self;
}

impl ProgramSubCommands for App<'_, '_> {
    fn program_subcommands(self) -> Self {
        self.subcommand(
            SubCommand::with_name("program")
                .about("Program management")
                .setting(AppSettings::SubcommandRequiredElseHelp)
                .arg(
                    Arg::with_name("skip_fee_check")
                        .long("skip-fee-check")
                        .hidden(hidden_unless_forced())
                        .takes_value(false)
                        .global(true),
                )
                .subcommand(
                    SubCommand::with_name("deploy")
                        .about("Deploy an upgradeable program")
                        .arg(
                            Arg::with_name("program_location")
                                .index(1)
                                .value_name("PROGRAM_FILEPATH")
                                .takes_value(true)
                                .help("/path/to/program.so"),
                        )
                        .arg(fee_payer_arg())
                        .arg(
                            Arg::with_name("buffer")
                                .long("buffer")
                                .value_name("BUFFER_SIGNER")
                                .takes_value(true)
                                .validator(is_valid_signer)
                                .help(
                                    "Intermediate buffer account to write data to, which can be \
                                     used to resume a failed deploy [default: random address]",
                                ),
                        )
                        .arg(
                            Arg::with_name("upgrade_authority")
                                .long("upgrade-authority")
                                .value_name("UPGRADE_AUTHORITY_SIGNER")
                                .takes_value(true)
                                .validator(is_valid_signer)
                                .help(
                                    "Upgrade authority [default: the default configured keypair]",
                                ),
                        )
                        .arg(pubkey!(
                            Arg::with_name("program_id")
                                .long("program-id")
                                .value_name("PROGRAM_ID"),
                            "Executable program; must be a signer for initial deploys, \
                             can be an address for upgrades [default: address of keypair at \
                             /path/to/program-keypair.json if present, otherwise a random address]."
                        ))
                        .arg(
                            Arg::with_name("final")
                                .long("final")
                                .help("The program will not be upgradeable"),
                        )
                        .arg(
                            Arg::with_name("max_len")
                                .long("max-len")
                                .value_name("max_len")
                                .takes_value(true)
                                .required(false)
                                .help(
                                    "Maximum length of the upgradeable program \
                                    [default: the length of the original deployed program]",
                                ),
                        )
                        .arg(
                            Arg::with_name("allow_excessive_balance")
                                .long("allow-excessive-deploy-account-balance")
                                .takes_value(false)
                                .help(
                                    "Use the designated program id even if the account already \
                                     holds a large balance of TACHYON",
                                ),
                        ),
                )
                .subcommand(
                    SubCommand::with_name("upgrade")
                        .about("Upgrade an upgradeable program")
                        .arg(pubkey!(
                            Arg::with_name("buffer")
                                .index(1)
                                .required(true)
                                .value_name("BUFFER_PUBKEY"),
                            "Intermediate buffer account with new program data"
                        ))
                        .arg(pubkey!(
                            Arg::with_name("program_id")
                                .index(2)
                                .required(true)
                                .value_name("PROGRAM_ID"),
                            "Executable program's address (pubkey)"
                        ))
                        .arg(fee_payer_arg())
                        .arg(
                            Arg::with_name("upgrade_authority")
                                .long("upgrade-authority")
                                .value_name("UPGRADE_AUTHORITY_SIGNER")
                                .takes_value(true)
                                .validator(is_valid_signer)
                                .help(
                                    "Upgrade authority [default: the default configured keypair]",
                                ),
                        )
                        .offline_args(),
                )
                .subcommand(
                    SubCommand::with_name("write-buffer")
                        .about("Writes a program into a buffer account")
                        .arg(
                            Arg::with_name("program_location")
                                .index(1)
                                .value_name("PROGRAM_FILEPATH")
                                .takes_value(true)
                                .required(true)
                                .help("/path/to/program.so"),
                        )
                        .arg(fee_payer_arg())
                        .arg(
                            Arg::with_name("buffer")
                                .long("buffer")
                                .value_name("BUFFER_SIGNER")
                                .takes_value(true)
                                .validator(is_valid_signer)
                                .help(
                                    "Buffer account to write data into [default: random address]",
                                ),
                        )
                        .arg(
                            Arg::with_name("buffer_authority")
                                .long("buffer-authority")
                                .value_name("BUFFER_AUTHORITY_SIGNER")
                                .takes_value(true)
                                .validator(is_valid_signer)
                                .help("Buffer authority [default: the default configured keypair]"),
                        )
                        .arg(
                            Arg::with_name("max_len")
                                .long("max-len")
                                .value_name("max_len")
                                .takes_value(true)
                                .required(false)
                                .help(
                                    "Maximum length of the upgradeable program \
                                    [default: the length of the original deployed program]",
                                ),
                        ),
                )
                .subcommand(
                    SubCommand::with_name("set-buffer-authority")
                        .about("Set a new buffer authority")
                        .arg(
                            Arg::with_name("buffer")
                                .index(1)
                                .value_name("BUFFER_PUBKEY")
                                .takes_value(true)
                                .required(true)
                                .help("Public key of the buffer"),
                        )
                        .arg(
                            Arg::with_name("buffer_authority")
                                .long("buffer-authority")
                                .value_name("BUFFER_AUTHORITY_SIGNER")
                                .takes_value(true)
                                .validator(is_valid_signer)
                                .help("Buffer authority [default: the default configured keypair]"),
                        )
                        .arg(pubkey!(
                            Arg::with_name("new_buffer_authority")
                                .long("new-buffer-authority")
                                .value_name("NEW_BUFFER_AUTHORITY")
                                .required(true),
                            "New buffer authority."
                        )),
                )
                .subcommand(
                    SubCommand::with_name("set-upgrade-authority")
                        .about("Set a new program authority")
                        .arg(
                            Arg::with_name("program_id")
                                .index(1)
                                .value_name("PROGRAM_ADDRESS")
                                .takes_value(true)
                                .required(true)
                                .help("Address of the program to upgrade"),
                        )
                        .arg(
                            Arg::with_name("upgrade_authority")
                                .long("upgrade-authority")
                                .value_name("UPGRADE_AUTHORITY_SIGNER")
                                .takes_value(true)
                                .validator(is_valid_signer)
                                .help(
                                    "Upgrade authority [default: the default configured keypair]",
                                ),
                        )
                        .arg(
                            Arg::with_name("new_upgrade_authority")
                                .long("new-upgrade-authority")
                                .value_name("NEW_UPGRADE_AUTHORITY")
                                .required_unless("final")
                                .takes_value(true)
                                .help(
                                    "New upgrade authority (keypair or pubkey). It is strongly \
                                     recommended to pass in a keypair to prevent mistakes in \
                                     setting the upgrade authority. You can opt out of this \
                                     behavior by passing \
                                     --skip-new-upgrade-authority-signer-check if you are really \
                                     confident that you are setting the correct authority. \
                                     Alternatively, If you wish to make the program immutable, \
                                     you should ignore this arg and pass the --final flag.",
                                ),
                        )
                        .arg(
                            Arg::with_name("final")
                                .long("final")
                                .conflicts_with("new_upgrade_authority")
                                .help("The program will not be upgradeable"),
                        )
                        .arg(
                            Arg::with_name("skip_new_upgrade_authority_signer_check")
                                .long("skip-new-upgrade-authority-signer-check")
                                .requires("new_upgrade_authority")
                                .takes_value(false)
                                .help(
                                    "Set this flag if you don't want the new authority to sign \
                                     the set-upgrade-authority transaction.",
                                ),
                        )
                        .offline_args(),
                )
                .subcommand(
                    SubCommand::with_name("show")
                        .about("Display information about a buffer or program")
                        .arg(
                            Arg::with_name("account")
                                .index(1)
                                .value_name("ACCOUNT_ADDRESS")
                                .takes_value(true)
                                .help("Address of the buffer or program to show"),
                        )
                        .arg(
                            Arg::with_name("programs")
                                .long("programs")
                                .conflicts_with("account")
                                .conflicts_with("buffers")
                                .required_unless_one(&["account", "buffers"])
                                .help("Show every upgradeable program that matches the authority"),
                        )
                        .arg(
                            Arg::with_name("buffers")
                                .long("buffers")
                                .conflicts_with("account")
                                .conflicts_with("programs")
                                .required_unless_one(&["account", "programs"])
                                .help("Show every upgradeable buffer that matches the authority"),
                        )
                        .arg(
                            Arg::with_name("all")
                                .long("all")
                                .conflicts_with("account")
                                .conflicts_with("buffer_authority")
                                .help("Show accounts for all authorities"),
                        )
                        .arg(pubkey!(
                            Arg::with_name("buffer_authority")
                                .long("buffer-authority")
                                .value_name("AUTHORITY")
                                .conflicts_with("all"),
                            "Authority [default: the default configured keypair]."
                        ))
                        .arg(
                            Arg::with_name("lamports")
                                .long("lamports")
                                .takes_value(false)
                                .help("Display balance in lamports instead of TACHYON"),
                        ),
                )
                .subcommand(
                    SubCommand::with_name("dump")
                        .about("Write the program data to a file")
                        .arg(
                            Arg::with_name("account")
                                .index(1)
                                .value_name("ACCOUNT_ADDRESS")
                                .takes_value(true)
                                .required(true)
                                .help("Address of the buffer or program"),
                        )
                        .arg(
                            Arg::with_name("output_location")
                                .index(2)
                                .value_name("OUTPUT_FILEPATH")
                                .takes_value(true)
                                .required(true)
                                .help("/path/to/program.so"),
                        ),
                )
                .subcommand(
                    SubCommand::with_name("close")
                        .about("Close a program or buffer account and withdraw all lamports")
                        .arg(
                            Arg::with_name("account")
                                .index(1)
                                .value_name("ACCOUNT_ADDRESS")
                                .takes_value(true)
                                .help("Address of the program or buffer account to close"),
                        )
                        .arg(
                            Arg::with_name("buffers")
                                .long("buffers")
                                .conflicts_with("account")
                                .required_unless("account")
                                .help("Close all buffer accounts that match the authority"),
                        )
                        .arg(
                            Arg::with_name("authority")
                                .long("authority")
                                .alias("buffer-authority")
                                .value_name("AUTHORITY_SIGNER")
                                .takes_value(true)
                                .validator(is_valid_signer)
                                .help(
                                    "Upgrade or buffer authority [default: the default configured \
                                     keypair]",
                                ),
                        )
                        .arg(pubkey!(
                            Arg::with_name("recipient_account")
                                .long("recipient")
                                .value_name("RECIPIENT_ADDRESS"),
                            "Recipient of closed account's lamports \
                             [default: the default configured keypair]."
                        ))
                        .arg(
                            Arg::with_name("lamports")
                                .long("lamports")
                                .takes_value(false)
                                .help("Display balance in lamports instead of TACHYON"),
                        )
                        .arg(
                            Arg::with_name("bypass_warning")
                                .long("bypass-warning")
                                .takes_value(false)
                                .help("Bypass the permanent program closure warning"),
                        ),
                )
                .subcommand(
                    SubCommand::with_name("extend")
                        .about(
                            "Extend the length of an upgradeable program to deploy larger programs",
                        )
                        .arg(
                            Arg::with_name("program_id")
                                .index(1)
                                .value_name("PROGRAM_ID")
                                .takes_value(true)
                                .required(true)
                                .validator(is_valid_pubkey)
                                .help("Address of the program to extend"),
                        )
                        .arg(
                            Arg::with_name("additional_bytes")
                                .index(2)
                                .value_name("ADDITIONAL_BYTES")
                                .takes_value(true)
                                .required(true)
                                .validator(is_parsable::<u32>)
                                .help(
                                    "Number of bytes that will be allocated for the program's \
                                     data account",
                                ),
                        ),
                ),
        )
        .subcommand(
            SubCommand::with_name("deploy")
                .about(
                    "Deploy has been removed. Use `Alembic program deploy` instead to deploy \
                     upgradeable programs",
                )
                .setting(AppSettings::Hidden),
        )
    }
}

pub fn parse_program_subcommand(
    matches: &ArgMatches<'_>,
    default_signer: &DefaultSigner,
    wallet_manager: &mut Option<Rc<RemoteWalletManager>>,
) -> Result<CliCommandInfo, CliError> {
    let (subcommand, sub_matches) = matches.subcommand();
    let matches_skip_fee_check = matches.is_present("skip_fee_check");
    let sub_matches_skip_fee_check = sub_matches
        .map(|m| m.is_present("skip_fee_check"))
        .unwrap_or(false);
    let skip_fee_check = matches_skip_fee_check || sub_matches_skip_fee_check;

    let response = match (subcommand, sub_matches) {
        ("deploy", Some(matches)) => {
            let (fee_payer, fee_payer_pubkey) =
                signer_of(matches, FEE_PAYER_ARG.name, wallet_manager)?;

            let mut bulk_signers = vec![
                Some(default_signer.signer_from_path(matches, wallet_manager)?),
                fee_payer, // if None, default signer will be supplied
            ];

            let program_location = matches
                .value_of("program_location")
                .map(|location| location.to_string());

            let buffer_pubkey = if let Ok((buffer_signer, Some(buffer_pubkey))) =
                signer_of(matches, "buffer", wallet_manager)
            {
                bulk_signers.push(buffer_signer);
                Some(buffer_pubkey)
            } else {
                pubkey_of_signer(matches, "buffer", wallet_manager)?
            };

            let program_pubkey = if let Ok((program_signer, Some(program_pubkey))) =
                signer_of(matches, "program_id", wallet_manager)
            {
                bulk_signers.push(program_signer);
                Some(program_pubkey)
            } else {
                pubkey_of_signer(matches, "program_id", wallet_manager)?
            };

            let (upgrade_authority, upgrade_authority_pubkey) =
                signer_of(matches, "upgrade_authority", wallet_manager)?;
            bulk_signers.push(upgrade_authority);

            let max_len = value_of(matches, "max_len");

            let signer_info =
                default_signer.generate_unique_signers(bulk_signers, matches, wallet_manager)?;

            CliCommandInfo {
                command: CliCommand::Program(ProgramCliCommand::Deploy {
                    program_location,
                    fee_payer_signer_index: signer_info.index_of(fee_payer_pubkey).unwrap(),
                    program_signer_index: signer_info.index_of_or_none(program_pubkey),
                    program_pubkey,
                    buffer_signer_index: signer_info.index_of_or_none(buffer_pubkey),
                    buffer_pubkey,
                    upgrade_authority_signer_index: signer_info
                        .index_of(upgrade_authority_pubkey)
                        .unwrap(),
                    is_final: matches.is_present("final"),
                    max_len,
                    allow_excessive_balance: matches.is_present("allow_excessive_balance"),
                    skip_fee_check,
                }),
                signers: signer_info.signers,
            }
        }
        ("upgrade", Some(matches)) => {
            let sign_only = matches.is_present(SIGN_ONLY_ARG.name);
            let dump_transaction_message = matches.is_present(DUMP_TRANSACTION_MESSAGE.name);
            let blockhash_query = BlockhashQuery::new_from_matches(matches);
            let buffer_pubkey = pubkey_of_signer(matches, "buffer", wallet_manager)
                .unwrap()
                .unwrap();
            let program_pubkey = pubkey_of_signer(matches, "program_id", wallet_manager)
                .unwrap()
                .unwrap();

            let (fee_payer, fee_payer_pubkey) =
                signer_of(matches, FEE_PAYER_ARG.name, wallet_manager)?;

            let mut bulk_signers = vec![
                fee_payer, // if None, default signer will be supplied
            ];

            let (upgrade_authority, upgrade_authority_pubkey) =
                signer_of(matches, "upgrade_authority", wallet_manager)?;
            bulk_signers.push(upgrade_authority);

            let signer_info =
                default_signer.generate_unique_signers(bulk_signers, matches, wallet_manager)?;

            CliCommandInfo {
                command: CliCommand::Program(ProgramCliCommand::Upgrade {
                    fee_payer_signer_index: signer_info.index_of(fee_payer_pubkey).unwrap(),
                    program_pubkey,
                    buffer_pubkey,
                    upgrade_authority_signer_index: signer_info
                        .index_of(upgrade_authority_pubkey)
                        .unwrap(),
                    sign_only,
                    dump_transaction_message,
                    blockhash_query,
                }),
                signers: signer_info.signers,
            }
        }
        ("write-buffer", Some(matches)) => {
            let (fee_payer, fee_payer_pubkey) =
                signer_of(matches, FEE_PAYER_ARG.name, wallet_manager)?;

            let mut bulk_signers = vec![
                Some(default_signer.signer_from_path(matches, wallet_manager)?),
                fee_payer, // if None, default signer will be supplied
            ];

            let buffer_pubkey = if let Ok((buffer_signer, Some(buffer_pubkey))) =
                signer_of(matches, "buffer", wallet_manager)
            {
                bulk_signers.push(buffer_signer);
                Some(buffer_pubkey)
            } else {
                pubkey_of_signer(matches, "buffer", wallet_manager)?
            };

            let (buffer_authority, buffer_authority_pubkey) =
                signer_of(matches, "buffer_authority", wallet_manager)?;
            bulk_signers.push(buffer_authority);

            let max_len = value_of(matches, "max_len");

            let signer_info =
                default_signer.generate_unique_signers(bulk_signers, matches, wallet_manager)?;

            CliCommandInfo {
                command: CliCommand::Program(ProgramCliCommand::WriteBuffer {
                    program_location: matches.value_of("program_location").unwrap().to_string(),
                    fee_payer_signer_index: signer_info.index_of(fee_payer_pubkey).unwrap(),
                    buffer_signer_index: signer_info.index_of_or_none(buffer_pubkey),
                    buffer_pubkey,
                    buffer_authority_signer_index: signer_info
                        .index_of(buffer_authority_pubkey)
                        .unwrap(),
                    max_len,
                    skip_fee_check,
                }),
                signers: signer_info.signers,
            }
        }
        ("set-buffer-authority", Some(matches)) => {
            let buffer_pubkey = pubkey_of(matches, "buffer").unwrap();

            let (buffer_authority_signer, buffer_authority_pubkey) =
                signer_of(matches, "buffer_authority", wallet_manager)?;
            let new_buffer_authority =
                pubkey_of_signer(matches, "new_buffer_authority", wallet_manager)?.unwrap();

            let signer_info = default_signer.generate_unique_signers(
                vec![
                    Some(default_signer.signer_from_path(matches, wallet_manager)?),
                    buffer_authority_signer,
                ],
                matches,
                wallet_manager,
            )?;

            CliCommandInfo {
                command: CliCommand::Program(ProgramCliCommand::SetBufferAuthority {
                    buffer_pubkey,
                    buffer_authority_index: signer_info.index_of(buffer_authority_pubkey),
                    new_buffer_authority,
                }),
                signers: signer_info.signers,
            }
        }
        ("set-upgrade-authority", Some(matches)) => {
            let sign_only = matches.is_present(SIGN_ONLY_ARG.name);
            let dump_transaction_message = matches.is_present(DUMP_TRANSACTION_MESSAGE.name);
            let blockhash_query = BlockhashQuery::new_from_matches(matches);
            let (upgrade_authority_signer, upgrade_authority_pubkey) =
                signer_of(matches, "upgrade_authority", wallet_manager)?;
            let program_pubkey = pubkey_of(matches, "program_id").unwrap();
            let is_final = matches.is_present("final");
            let new_upgrade_authority = if is_final {
                None
            } else {
                pubkey_of_signer(matches, "new_upgrade_authority", wallet_manager)?
            };

            let mut signers = vec![
                Some(default_signer.signer_from_path(matches, wallet_manager)?),
                upgrade_authority_signer,
            ];

            if !is_final && !matches.is_present("skip_new_upgrade_authority_signer_check") {
                let (new_upgrade_authority_signer, _) =
                    signer_of(matches, "new_upgrade_authority", wallet_manager)?;
                signers.push(new_upgrade_authority_signer);
            }

            let signer_info =
                default_signer.generate_unique_signers(signers, matches, wallet_manager)?;

            if matches.is_present("skip_new_upgrade_authority_signer_check") || is_final {
                CliCommandInfo {
                    command: CliCommand::Program(ProgramCliCommand::SetUpgradeAuthority {
                        program_pubkey,
                        upgrade_authority_index: signer_info.index_of(upgrade_authority_pubkey),
                        new_upgrade_authority,
                        sign_only,
                        dump_transaction_message,
                        blockhash_query,
                    }),
                    signers: signer_info.signers,
                }
            } else {
                CliCommandInfo {
                    command: CliCommand::Program(ProgramCliCommand::SetUpgradeAuthorityChecked {
                        program_pubkey,
                        upgrade_authority_index: signer_info
                            .index_of(upgrade_authority_pubkey)
                            .expect("upgrade authority is missing from signers"),
                        new_upgrade_authority_index: signer_info
                            .index_of(new_upgrade_authority)
                            .expect("new upgrade authority is missing from signers"),
                        sign_only,
                        dump_transaction_message,
                        blockhash_query,
                    }),
                    signers: signer_info.signers,
                }
            }
        }
        ("show", Some(matches)) => {
            let authority_pubkey = if let Some(authority_pubkey) =
                pubkey_of_signer(matches, "buffer_authority", wallet_manager)?
            {
                authority_pubkey
            } else {
                default_signer
                    .signer_from_path(matches, wallet_manager)?
                    .pubkey()
            };

            CliCommandInfo {
                command: CliCommand::Program(ProgramCliCommand::Show {
                    account_pubkey: pubkey_of(matches, "account"),
                    authority_pubkey,
                    get_programs: matches.is_present("programs"),
                    get_buffers: matches.is_present("buffers"),
                    all: matches.is_present("all"),
                    use_lamports_unit: matches.is_present("lamports"),
                }),
                signers: vec![],
            }
        }
        ("dump", Some(matches)) => CliCommandInfo {
            command: CliCommand::Program(ProgramCliCommand::Dump {
                account_pubkey: pubkey_of(matches, "account"),
                output_location: matches.value_of("output_location").unwrap().to_string(),
            }),
            signers: vec![],
        },
        ("close", Some(matches)) => {
            let account_pubkey = if matches.is_present("buffers") {
                None
            } else {
                pubkey_of(matches, "account")
            };

            let recipient_pubkey = if let Some(recipient_pubkey) =
                pubkey_of_signer(matches, "recipient_account", wallet_manager)?
            {
                recipient_pubkey
            } else {
                default_signer
                    .signer_from_path(matches, wallet_manager)?
                    .pubkey()
            };

            let (authority_signer, authority_pubkey) =
                signer_of(matches, "authority", wallet_manager)?;

            let signer_info = default_signer.generate_unique_signers(
                vec![
                    Some(default_signer.signer_from_path(matches, wallet_manager)?),
                    authority_signer,
                ],
                matches,
                wallet_manager,
            )?;

            CliCommandInfo {
                command: CliCommand::Program(ProgramCliCommand::Close {
                    account_pubkey,
                    recipient_pubkey,
                    authority_index: signer_info.index_of(authority_pubkey).unwrap(),
                    use_lamports_unit: matches.is_present("lamports"),
                    bypass_warning: matches.is_present("bypass_warning"),
                }),
                signers: signer_info.signers,
            }
        }
        ("extend", Some(matches)) => {
            let program_pubkey = pubkey_of(matches, "program_id").unwrap();
            let additional_bytes = value_of(matches, "additional_bytes").unwrap();

            let signer_info = default_signer.generate_unique_signers(
                vec![Some(
                    default_signer.signer_from_path(matches, wallet_manager)?,
                )],
                matches,
                wallet_manager,
            )?;

            CliCommandInfo {
                command: CliCommand::Program(ProgramCliCommand::ExtendProgram {
                    program_pubkey,
                    additional_bytes,
                }),
                signers: signer_info.signers,
            }
        }
        _ => unreachable!(),
    };
    Ok(response)
}

pub fn process_program_subcommand(
    rpc_client: Arc<RpcClient>,
    config: &CliConfig,
    program_subcommand: &ProgramCliCommand,
) -> ProcessResult {
    match program_subcommand {
        ProgramCliCommand::Deploy {
            program_location,
            fee_payer_signer_index,
            program_signer_index,
            program_pubkey,
            buffer_signer_index,
            buffer_pubkey,
            upgrade_authority_signer_index,
            is_final,
            max_len,
            allow_excessive_balance,
            skip_fee_check,
        } => process_program_deploy(
            rpc_client,
            config,
            program_location,
            *fee_payer_signer_index,
            *program_signer_index,
            *program_pubkey,
            *buffer_signer_index,
            *buffer_pubkey,
            *upgrade_authority_signer_index,
            *is_final,
            *max_len,
            *allow_excessive_balance,
            *skip_fee_check,
        ),
        ProgramCliCommand::Upgrade {
            fee_payer_signer_index,
            program_pubkey,
            buffer_pubkey,
            upgrade_authority_signer_index,
            sign_only,
            dump_transaction_message,
            blockhash_query,
        } => process_program_upgrade(
            rpc_client,
            config,
            *fee_payer_signer_index,
            *program_pubkey,
            *buffer_pubkey,
            *upgrade_authority_signer_index,
            *sign_only,
            *dump_transaction_message,
            blockhash_query,
        ),
        ProgramCliCommand::WriteBuffer {
            program_location,
            fee_payer_signer_index,
            buffer_signer_index,
            buffer_pubkey,
            buffer_authority_signer_index,
            max_len,
            skip_fee_check,
        } => process_write_buffer(
            rpc_client,
            config,
            program_location,
            *fee_payer_signer_index,
            *buffer_signer_index,
            *buffer_pubkey,
            *buffer_authority_signer_index,
            *max_len,
            *skip_fee_check,
        ),
        ProgramCliCommand::SetBufferAuthority {
            buffer_pubkey,
            buffer_authority_index,
            new_buffer_authority,
        } => process_set_authority(
            &rpc_client,
            config,
            None,
            Some(*buffer_pubkey),
            *buffer_authority_index,
            Some(*new_buffer_authority),
            false,
            false,
            &BlockhashQuery::default(),
        ),
        ProgramCliCommand::SetUpgradeAuthority {
            program_pubkey,
            upgrade_authority_index,
            new_upgrade_authority,
            sign_only,
            dump_transaction_message,
            blockhash_query,
        } => process_set_authority(
            &rpc_client,
            config,
            Some(*program_pubkey),
            None,
            *upgrade_authority_index,
            *new_upgrade_authority,
            *sign_only,
            *dump_transaction_message,
            blockhash_query,
        ),
        ProgramCliCommand::SetUpgradeAuthorityChecked {
            program_pubkey,
            upgrade_authority_index,
            new_upgrade_authority_index,
            sign_only,
            dump_transaction_message,
            blockhash_query,
        } => process_set_authority_checked(
            &rpc_client,
            config,
            *program_pubkey,
            *upgrade_authority_index,
            *new_upgrade_authority_index,
            *sign_only,
            *dump_transaction_message,
            blockhash_query,
        ),
        ProgramCliCommand::Show {
            account_pubkey,
            authority_pubkey,
            get_programs,
            get_buffers,
            all,
            use_lamports_unit,
        } => process_show(
            &rpc_client,
            config,
            *account_pubkey,
            *authority_pubkey,
            *get_programs,
            *get_buffers,
            *all,
            *use_lamports_unit,
        ),
        ProgramCliCommand::Dump {
            account_pubkey,
            output_location,
        } => process_dump(&rpc_client, config, *account_pubkey, output_location),
        ProgramCliCommand::Close {
            account_pubkey,
            recipient_pubkey,
            authority_index,
            use_lamports_unit,
            bypass_warning,
        } => process_close(
            &rpc_client,
            config,
            *account_pubkey,
            *recipient_pubkey,
            *authority_index,
            *use_lamports_unit,
            *bypass_warning,
        ),
        ProgramCliCommand::ExtendProgram {
            program_pubkey,
            additional_bytes,
        } => process_extend_program(&rpc_client, config, *program_pubkey, *additional_bytes),
    }
}

fn get_default_program_keypair(program_location: &Option<String>) -> Keypair {
    let program_keypair = {
        if let Some(program_location) = program_location {
            let mut keypair_file = PathBuf::new();
            keypair_file.push(program_location);
            let mut filename = keypair_file.file_stem().unwrap().to_os_string();
            filename.push("-keypair");
            keypair_file.set_file_name(filename);
            keypair_file.set_extension("json");
            if let Ok(keypair) = read_keypair_file(keypair_file.to_str().unwrap()) {
                keypair
            } else {
                Keypair::new()
            }
        } else {
            Keypair::new()
        }
    };
    program_keypair
}

/// Deploy program using upgradeable loader. It also can process program upgrades
#[allow(clippy::too_many_arguments)]
fn process_program_deploy(
    rpc_client: Arc<RpcClient>,
    config: &CliConfig,
    program_location: &Option<String>,
    fee_payer_signer_index: SignerIndex,
    program_signer_index: Option<SignerIndex>,
    program_pubkey: Option<Pubkey>,
    buffer_signer_index: Option<SignerIndex>,
    buffer_pubkey: Option<Pubkey>,
    upgrade_authority_signer_index: SignerIndex,
    is_final: bool,
    max_len: Option<usize>,
    allow_excessive_balance: bool,
    skip_fee_check: bool,
) -> ProcessResult {
    let fee_payer_signer = config.signers[fee_payer_signer_index];
    let upgrade_authority_signer = config.signers[upgrade_authority_signer_index];

    let (buffer_words, buffer_mnemonic, buffer_keypair) = create_ephemeral_keypair()?;
    let (buffer_provided, buffer_signer, buffer_pubkey) = if let Some(i) = buffer_signer_index {
        (true, Some(config.signers[i]), config.signers[i].pubkey())
    } else if let Some(pubkey) = buffer_pubkey {
        (true, None, pubkey)
    } else {
        (
            false,
            Some(&buffer_keypair as &dyn Signer),
            buffer_keypair.pubkey(),
        )
    };

    let default_program_keypair = get_default_program_keypair(program_location);
    let (program_signer, program_pubkey) = if let Some(i) = program_signer_index {
        (Some(config.signers[i]), config.signers[i].pubkey())
    } else if let Some(program_pubkey) = program_pubkey {
        (None, program_pubkey)
    } else {
        (
            Some(&default_program_keypair as &dyn Signer),
            default_program_keypair.pubkey(),
        )
    };

    let do_initial_deploy = if let Some(account) = rpc_client
        .get_account_with_commitment(&program_pubkey, config.commitment)?
        .value
    {
        if account.owner != bpf_loader_upgradeable::id() {
            return Err(format!(
                "Account {program_pubkey} is not an upgradeable program or already in use"
            )
            .into());
        }

        if !account.executable {
            // Continue an initial deploy
            true
        } else if let Ok(UpgradeableLoaderState::Program {
            programdata_address,
        }) = account.state()
        {
            if let Some(account) = rpc_client
                .get_account_with_commitment(&programdata_address, config.commitment)?
                .value
            {
                if let Ok(UpgradeableLoaderState::ProgramData {
                    slot: _,
                    upgrade_authority_address: program_authority_pubkey,
                }) = account.state()
                {
                    if program_authority_pubkey.is_none() {
                        return Err(
                            format!("Program {program_pubkey} is no longer upgradeable").into()
                        );
                    }
                    if program_authority_pubkey != Some(upgrade_authority_signer.pubkey()) {
                        return Err(format!(
                            "Program's authority {:?} does not match authority provided {:?}",
                            program_authority_pubkey,
                            upgrade_authority_signer.pubkey(),
                        )
                        .into());
                    }
                    // Do upgrade
                    false
                } else {
                    return Err(format!(
                        "Program {program_pubkey} has been closed, use a new Program Id"
                    )
                    .into());
                }
            } else {
                return Err(format!(
                    "Program {program_pubkey} has been closed, use a new Program Id"
                )
                .into());
            }
        } else {
            return Err(format!("{program_pubkey} is not an upgradeable program").into());
        }
    } else {
        // do new deploy
        true
    };

    let (program_data, program_len) = if let Some(program_location) = program_location {
        let program_data = read_and_verify_elf(program_location)?;
        let program_len = program_data.len();
        (program_data, program_len)
    } else if buffer_provided {
        (
            vec![],
            fetch_buffer_len(&rpc_client, config, buffer_pubkey)?,
        )
    } else {
        return Err("Program location required if buffer not supplied".into());
    };
    let program_data_max_len = if let Some(len) = max_len {
        if program_len > len {
            return Err(
                "Max length specified not large enough to accommodate desired program".into(),
            );
        }
        len
    } else {
        program_len
    };

    let min_rent_exempt_program_data_balance = rpc_client.get_minimum_balance_for_rent_exemption(
        UpgradeableLoaderState::size_of_programdata(program_data_max_len),
    )?;

    let result = if do_initial_deploy {
        if program_signer.is_none() {
            return Err(
                "Initial deployments require a keypair be provided for the program id".into(),
            );
        }
        do_process_program_write_and_deploy(
            rpc_client.clone(),
            config,
            &program_data,
            program_len,
            program_data_max_len,
            min_rent_exempt_program_data_balance,
            &bpf_loader_upgradeable::id(),
            fee_payer_signer,
            Some(&[program_signer.unwrap(), upgrade_authority_signer]),
            buffer_signer,
            &buffer_pubkey,
            upgrade_authority_signer,
            allow_excessive_balance,
            skip_fee_check,
        )
    } else {
        do_process_program_upgrade(
            rpc_client.clone(),
            config,
            &program_data,
            program_len,
            min_rent_exempt_program_data_balance,
            fee_payer_signer,
            &program_pubkey,
            upgrade_authority_signer,
            &buffer_pubkey,
            buffer_signer,
            skip_fee_check,
        )
    };
    if result.is_ok() && is_final {
        process_set_authority(
            &rpc_client,
            config,
            Some(program_pubkey),
            None,
            Some(upgrade_authority_signer_index),
            None,
            false,
            false,
            &BlockhashQuery::default(),
        )?;
    }
    if result.is_err() && !buffer_provided {
        // We might have deployed "temporary" buffer but failed to deploy our program from this
        // buffer, reporting this to the user - so he can retry deploying re-using same buffer.
        report_ephemeral_mnemonic(buffer_words, buffer_mnemonic);
    }
    result
}

fn fetch_buffer_len(
    rpc_client: &RpcClient,
    config: &CliConfig,
    buffer_pubkey: Pubkey,
) -> Result<usize, Box<dyn std::error::Error>> {
    // Check supplied buffer account
    if let Some(account) = rpc_client
        .get_account_with_commitment(&buffer_pubkey, config.commitment)?
        .value
    {
        if !bpf_loader_upgradeable::check_id(&account.owner) {
            return Err(format!(
                "Buffer account {buffer_pubkey} is not owned by the BPF Upgradeable Loader",
            )
            .into());
        }

        match account.state() {
            Ok(UpgradeableLoaderState::Buffer { .. }) => {
                // continue if buffer is initialized
            }
            Ok(UpgradeableLoaderState::Program { .. }) => {
                return Err(format!("Cannot use program account {buffer_pubkey} as buffer").into());
            }
            Ok(UpgradeableLoaderState::ProgramData { .. }) => {
                return Err(
                    format!("Cannot use program data account {buffer_pubkey} as buffer",).into(),
                )
            }
            Ok(UpgradeableLoaderState::Uninitialized) => {
                return Err(format!("Buffer account {buffer_pubkey} is not initialized").into());
            }
            Err(_) => {
                return Err(
                    format!("Buffer account {buffer_pubkey} could not be deserialized").into(),
                )
            }
        };

        let program_len = account
            .data
            .len()
            .saturating_sub(UpgradeableLoaderState::size_of_buffer_metadata());

        Ok(program_len)
    } else {
        Err(format!("Buffer account {buffer_pubkey} not found, was it already consumed?",).into())
    }
}

/// Upgrade existing program using upgradeable loader
#[allow(clippy::too_many_arguments)]
fn process_program_upgrade(
    rpc_client: Arc<RpcClient>,
    config: &CliConfig,
    fee_payer_signer_index: SignerIndex,
    program_id: Pubkey,
    buffer_pubkey: Pubkey,
    upgrade_authority_signer_index: SignerIndex,
    sign_only: bool,
    dump_transaction_message: bool,
    blockhash_query: &BlockhashQuery,
) -> ProcessResult {
    let fee_payer_signer = config.signers[fee_payer_signer_index];
    let upgrade_authority_signer = config.signers[upgrade_authority_signer_index];

    let blockhash = blockhash_query.get_blockhash(&rpc_client, config.commitment)?;
    let message = Message::new_with_blockhash(
        &[bpf_loader_upgradeable::upgrade(
            &program_id,
            &buffer_pubkey,
            &upgrade_authority_signer.pubkey(),
            &fee_payer_signer.pubkey(),
        )],
        Some(&fee_payer_signer.pubkey()),
        &blockhash,
    );

    if sign_only {
        let mut tx = Transaction::new_unsigned(message);
        let signers = &[fee_payer_signer, upgrade_authority_signer];
        // Using try_partial_sign here because fee_payer_signer might not be the fee payer we
        // end up using for this transaction (it might be NullSigner in `--sign-only` mode).
        tx.try_partial_sign(signers, blockhash)?;
        return_signers_with_config(
            &tx,
            &config.output_format,
            &ReturnSignersConfig {
                dump_transaction_message,
            },
        )
    } else {
        let fee = rpc_client.get_fee_for_message(&message)?;
        check_account_for_spend_and_fee_with_commitment(
            &rpc_client,
            &fee_payer_signer.pubkey(),
            0,
            fee,
            config.commitment,
        )?;
        let mut tx = Transaction::new_unsigned(message);
        let signers = &[fee_payer_signer, upgrade_authority_signer];
        tx.try_sign(signers, blockhash)?;
        let final_tx_sig = rpc_client
            .send_and_confirm_transaction_with_spinner(&tx)
            .map_err(|e| format!("Upgrading program failed: {e}"))?;
        let program_id = CliProgramId {
            program_id: program_id.to_string(),
            signature: Some(final_tx_sig.to_string()),
        };
        Ok(config.output_format.formatted_string(&program_id))
    }
}

fn process_write_buffer(
    rpc_client: Arc<RpcClient>,
    config: &CliConfig,
    program_location: &str,
    fee_payer_signer_index: SignerIndex,
    buffer_signer_index: Option<SignerIndex>,
    buffer_pubkey: Option<Pubkey>,
    buffer_authority_signer_index: SignerIndex,
    max_len: Option<usize>,
    skip_fee_check: bool,
) -> ProcessResult {
    let fee_payer_signer = config.signers[fee_payer_signer_index];
    let buffer_authority = config.signers[buffer_authority_signer_index];

    // Create ephemeral keypair to use for Buffer account, if not provided
    let (words, mnemonic, buffer_keypair) = create_ephemeral_keypair()?;
    let (buffer_signer, buffer_pubkey) = if let Some(i) = buffer_signer_index {
        (Some(config.signers[i]), config.signers[i].pubkey())
    } else if let Some(pubkey) = buffer_pubkey {
        (None, pubkey)
    } else {
        (
            Some(&buffer_keypair as &dyn Signer),
            buffer_keypair.pubkey(),
        )
    };

    if let Some(account) = rpc_client
        .get_account_with_commitment(&buffer_pubkey, config.commitment)?
        .value
    {
        if let Ok(UpgradeableLoaderState::Buffer { authority_address }) = account.state() {
            if authority_address.is_none() {
                return Err(format!("Buffer {buffer_pubkey} is immutable").into());
            }
            if authority_address != Some(buffer_authority.pubkey()) {
                return Err(format!(
                    "Buffer's authority {:?} does not match authority provided {}",
                    authority_address,
                    buffer_authority.pubkey()
                )
                .into());
            }
        } else {
            return Err(
                format!("{buffer_pubkey} is not an upgradeable loader buffer account").into(),
            );
        }
    }

    let program_data = read_and_verify_elf(program_location)?;
    let buffer_data_max_len = if let Some(len) = max_len {
        len
    } else {
        program_data.len()
    };
    let min_rent_exempt_program_data_balance = rpc_client.get_minimum_balance_for_rent_exemption(
        UpgradeableLoaderState::size_of_programdata(buffer_data_max_len),
    )?;

    let result = do_process_program_write_and_deploy(
        rpc_client,
        config,
        &program_data,
        program_data.len(),
        buffer_data_max_len,
        min_rent_exempt_program_data_balance,
        &bpf_loader_upgradeable::id(),
        fee_payer_signer,
        None,
        buffer_signer,
        &buffer_pubkey,
        buffer_authority,
        true,
        skip_fee_check,
    );
    if result.is_err() && buffer_signer_index.is_none() && buffer_signer.is_some() {
        report_ephemeral_mnemonic(words, mnemonic);
    }
    result
}

fn process_set_authority(
    rpc_client: &RpcClient,
    config: &CliConfig,
    program_pubkey: Option<Pubkey>,
    buffer_pubkey: Option<Pubkey>,
    authority: Option<SignerIndex>,
    new_authority: Option<Pubkey>,
    sign_only: bool,
    dump_transaction_message: bool,
    blockhash_query: &BlockhashQuery,
) -> ProcessResult {
    let authority_signer = if let Some(index) = authority {
        config.signers[index]
    } else {
        return Err("Set authority requires the current authority".into());
    };

    trace!("Set a new authority");
    let blockhash = blockhash_query.get_blockhash(rpc_client, config.commitment)?;

    let mut tx = if let Some(ref pubkey) = program_pubkey {
        Transaction::new_unsigned(Message::new(
            &[bpf_loader_upgradeable::set_upgrade_authority(
                pubkey,
                &authority_signer.pubkey(),
                new_authority.as_ref(),
            )],
            Some(&config.signers[0].pubkey()),
        ))
    } else if let Some(pubkey) = buffer_pubkey {
        if let Some(ref new_authority) = new_authority {
            Transaction::new_unsigned(Message::new(
                &[bpf_loader_upgradeable::set_buffer_authority(
                    &pubkey,
                    &authority_signer.pubkey(),
                    new_authority,
                )],
                Some(&config.signers[0].pubkey()),
            ))
        } else {
            return Err("Buffer authority cannot be None".into());
        }
    } else {
        return Err("Program or Buffer not provided".into());
    };

    let signers = &[config.signers[0], authority_signer];

    if sign_only {
        tx.try_partial_sign(signers, blockhash)?;
        return_signers_with_config(
            &tx,
            &config.output_format,
            &ReturnSignersConfig {
                dump_transaction_message,
            },
        )
    } else {
        tx.try_sign(signers, blockhash)?;
        rpc_client
            .send_and_confirm_transaction_with_spinner_and_config(
                &tx,
                config.commitment,
                RpcSendTransactionConfig {
                    preflight_commitment: Some(config.commitment.commitment),
                    ..RpcSendTransactionConfig::default()
                },
            )
            .map_err(|e| format!("Setting authority failed: {e}"))?;

        let authority = CliProgramAuthority {
            authority: new_authority
                .map(|pubkey| pubkey.to_string())
                .unwrap_or_else(|| "none".to_string()),
            account_type: if program_pubkey.is_some() {
                CliProgramAccountType::Program
            } else {
                CliProgramAccountType::Buffer
            },
        };
        Ok(config.output_format.formatted_string(&authority))
    }
}

fn process_set_authority_checked(
    rpc_client: &RpcClient,
    config: &CliConfig,
    program_pubkey: Pubkey,
    authority_index: SignerIndex,
    new_authority_index: SignerIndex,
    sign_only: bool,
    dump_transaction_message: bool,
    blockhash_query: &BlockhashQuery,
) -> ProcessResult {
    let authority_signer = config.signers[authority_index];
    let new_authority_signer = config.signers[new_authority_index];

    trace!("Set a new (checked) authority");
    let blockhash = blockhash_query.get_blockhash(rpc_client, config.commitment)?;

    let mut tx = Transaction::new_unsigned(Message::new(
        &[bpf_loader_upgradeable::set_upgrade_authority_checked(
            &program_pubkey,
            &authority_signer.pubkey(),
            &new_authority_signer.pubkey(),
        )],
        Some(&config.signers[0].pubkey()),
    ));

    let signers = &[config.signers[0], authority_signer, new_authority_signer];
    if sign_only {
        tx.try_partial_sign(signers, blockhash)?;
        return_signers_with_config(
            &tx,
            &config.output_format,
            &ReturnSignersConfig {
                dump_transaction_message,
            },
        )
    } else {
        tx.try_sign(signers, blockhash)?;
        rpc_client
            .send_and_confirm_transaction_with_spinner_and_config(
                &tx,
                config.commitment,
                RpcSendTransactionConfig {
                    preflight_commitment: Some(config.commitment.commitment),
                    ..RpcSendTransactionConfig::default()
                },
            )
            .map_err(|e| format!("Setting authority failed: {e}"))?;

        let authority = CliProgramAuthority {
            authority: new_authority_signer.pubkey().to_string(),
            account_type: CliProgramAccountType::Program,
        };
        Ok(config.output_format.formatted_string(&authority))
    }
}

const ACCOUNT_TYPE_SIZE: usize = 4;
const SLOT_SIZE: usize = size_of::<u64>();
const OPTION_SIZE: usize = 1;
const PUBKEY_LEN: usize = 32;

fn get_buffers(
    rpc_client: &RpcClient,
    authority_pubkey: Option<Pubkey>,
    use_lamports_unit: bool,
) -> Result<CliUpgradeableBuffers, Box<dyn std::error::Error>> {
    let mut filters = vec![RpcFilterType::Memcmp(Memcmp::new_base58_encoded(
        0,
        &[1, 0, 0, 0],
    ))];
    if let Some(authority_pubkey) = authority_pubkey {
        filters.push(RpcFilterType::Memcmp(Memcmp::new_base58_encoded(
            ACCOUNT_TYPE_SIZE,
            &[1],
        )));
        filters.push(RpcFilterType::Memcmp(Memcmp::new_base58_encoded(
            ACCOUNT_TYPE_SIZE + OPTION_SIZE,
            authority_pubkey.as_ref(),
        )));
    }

    let results = get_accounts_with_filter(
        rpc_client,
        filters,
        ACCOUNT_TYPE_SIZE + OPTION_SIZE + PUBKEY_LEN,
    )?;

    let mut buffers = vec![];
    for (address, account) in results.iter() {
        if let Ok(UpgradeableLoaderState::Buffer { authority_address }) = account.state() {
            buffers.push(CliUpgradeableBuffer {
                address: address.to_string(),
                authority: authority_address
                    .map(|pubkey| pubkey.to_string())
                    .unwrap_or_else(|| "none".to_string()),
                data_len: 0,
                lamports: account.lamports,
                use_lamports_unit,
            });
        } else {
            return Err(format!("Error parsing Buffer account {address}").into());
        }
    }
    Ok(CliUpgradeableBuffers {
        buffers,
        use_lamports_unit,
    })
}

fn get_programs(
    rpc_client: &RpcClient,
    authority_pubkey: Option<Pubkey>,
    use_lamports_unit: bool,
) -> Result<CliUpgradeablePrograms, Box<dyn std::error::Error>> {
    let mut filters = vec![RpcFilterType::Memcmp(Memcmp::new_base58_encoded(
        0,
        &[3, 0, 0, 0],
    ))];
    if let Some(authority_pubkey) = authority_pubkey {
        filters.push(RpcFilterType::Memcmp(Memcmp::new_base58_encoded(
            ACCOUNT_TYPE_SIZE + SLOT_SIZE,
            &[1],
        )));
        filters.push(RpcFilterType::Memcmp(Memcmp::new_base58_encoded(
            ACCOUNT_TYPE_SIZE + SLOT_SIZE + OPTION_SIZE,
            authority_pubkey.as_ref(),
        )));
    }

    let results = get_accounts_with_filter(
        rpc_client,
        filters,
        ACCOUNT_TYPE_SIZE + SLOT_SIZE + OPTION_SIZE + PUBKEY_LEN,
    )?;

    let mut programs = vec![];
    for (programdata_address, programdata_account) in results.iter() {
        if let Ok(UpgradeableLoaderState::ProgramData {
            slot,
            upgrade_authority_address,
        }) = programdata_account.state()
        {
            let mut bytes = vec![2, 0, 0, 0];
            bytes.extend_from_slice(programdata_address.as_ref());
            let filters = vec![RpcFilterType::Memcmp(Memcmp::new_base58_encoded(0, &bytes))];

            let results = get_accounts_with_filter(rpc_client, filters, 0)?;
            if results.len() != 1 {
                return Err(format!(
                    "Error: More than one Program associated with ProgramData account \
                     {programdata_address}"
                )
                .into());
            }
            programs.push(CliUpgradeableProgram {
                program_id: results[0].0.to_string(),
                owner: programdata_account.owner.to_string(),
                programdata_address: programdata_address.to_string(),
                authority: upgrade_authority_address
                    .map(|pubkey| pubkey.to_string())
                    .unwrap_or_else(|| "none".to_string()),
                last_deploy_slot: slot,
                data_len: programdata_account.data.len()
                    - UpgradeableLoaderState::size_of_programdata_metadata(),
                lamports: programdata_account.lamports,
                use_lamports_unit,
            });
        } else {
            return Err(format!("Error parsing ProgramData account {programdata_address}").into());
        }
    }
    Ok(CliUpgradeablePrograms {
        programs,
        use_lamports_unit,
    })
}

fn get_accounts_with_filter(
    rpc_client: &RpcClient,
    filters: Vec<RpcFilterType>,
    length: usize,
) -> Result<Vec<(Pubkey, Account)>, Box<dyn std::error::Error>> {
    let results = rpc_client.get_program_accounts_with_config(
        &bpf_loader_upgradeable::id(),
        RpcProgramAccountsConfig {
            filters: Some(filters),
            account_config: RpcAccountInfoConfig {
                encoding: Some(UiAccountEncoding::Base64),
                data_slice: Some(UiDataSliceConfig { offset: 0, length }),
                ..RpcAccountInfoConfig::default()
            },
            ..RpcProgramAccountsConfig::default()
        },
    )?;
    Ok(results)
}

fn process_show(
    rpc_client: &RpcClient,
    config: &CliConfig,
    account_pubkey: Option<Pubkey>,
    authority_pubkey: Pubkey,
    programs: bool,
    buffers: bool,
    all: bool,
    use_lamports_unit: bool,
) -> ProcessResult {
    if let Some(account_pubkey) = account_pubkey {
        if let Some(account) = rpc_client
            .get_account_with_commitment(&account_pubkey, config.commitment)?
            .value
        {
            if account.owner == bpf_loader::id() || account.owner == bpf_loader_deprecated::id() {
                Ok(config.output_format.formatted_string(&CliProgram {
                    program_id: account_pubkey.to_string(),
                    owner: account.owner.to_string(),
                    data_len: account.data.len(),
                }))
            } else if account.owner == bpf_loader_upgradeable::id() {
                if let Ok(UpgradeableLoaderState::Program {
                    programdata_address,
                }) = account.state()
                {
                    if let Some(programdata_account) = rpc_client
                        .get_account_with_commitment(&programdata_address, config.commitment)?
                        .value
                    {
                        if let Ok(UpgradeableLoaderState::ProgramData {
                            upgrade_authority_address,
                            slot,
                        }) = programdata_account.state()
                        {
                            Ok(config
                                .output_format
                                .formatted_string(&CliUpgradeableProgram {
                                    program_id: account_pubkey.to_string(),
                                    owner: account.owner.to_string(),
                                    programdata_address: programdata_address.to_string(),
                                    authority: upgrade_authority_address
                                        .map(|pubkey| pubkey.to_string())
                                        .unwrap_or_else(|| "none".to_string()),
                                    last_deploy_slot: slot,
                                    data_len: programdata_account.data.len()
                                        - UpgradeableLoaderState::size_of_programdata_metadata(),
                                    lamports: programdata_account.lamports,
                                    use_lamports_unit,
                                }))
                        } else {
                            Err(format!("Program {account_pubkey} has been closed").into())
                        }
                    } else {
                        Err(format!("Program {account_pubkey} has been closed").into())
                    }
                } else if let Ok(UpgradeableLoaderState::Buffer { authority_address }) =
                    account.state()
                {
                    Ok(config
                        .output_format
                        .formatted_string(&CliUpgradeableBuffer {
                            address: account_pubkey.to_string(),
                            authority: authority_address
                                .map(|pubkey| pubkey.to_string())
                                .unwrap_or_else(|| "none".to_string()),
                            data_len: account.data.len()
                                - UpgradeableLoaderState::size_of_buffer_metadata(),
                            lamports: account.lamports,
                            use_lamports_unit,
                        }))
                } else {
                    Err(format!(
                        "{account_pubkey} is not an upgradeable loader Buffer or Program account"
                    )
                    .into())
                }
            } else {
                Err(format!("{account_pubkey} is not an SBF program").into())
            }
        } else {
            Err(format!("Unable to find the account {account_pubkey}").into())
        }
    } else if programs {
        let authority_pubkey = if all { None } else { Some(authority_pubkey) };
        let programs = get_programs(rpc_client, authority_pubkey, use_lamports_unit)?;
        Ok(config.output_format.formatted_string(&programs))
    } else if buffers {
        let authority_pubkey = if all { None } else { Some(authority_pubkey) };
        let buffers = get_buffers(rpc_client, authority_pubkey, use_lamports_unit)?;
        Ok(config.output_format.formatted_string(&buffers))
    } else {
        Err("Invalid parameters".to_string().into())
    }
}

fn process_dump(
    rpc_client: &RpcClient,
    config: &CliConfig,
    account_pubkey: Option<Pubkey>,
    output_location: &str,
) -> ProcessResult {
    if let Some(account_pubkey) = account_pubkey {
        if let Some(account) = rpc_client
            .get_account_with_commitment(&account_pubkey, config.commitment)?
            .value
        {
            if account.owner == bpf_loader::id() || account.owner == bpf_loader_deprecated::id() {
                let mut f = File::create(output_location)?;
                f.write_all(&account.data)?;
                Ok(format!("Wrote program to {output_location}"))
            } else if account.owner == bpf_loader_upgradeable::id() {
                if let Ok(UpgradeableLoaderState::Program {
                    programdata_address,
                }) = account.state()
                {
                    if let Some(programdata_account) = rpc_client
                        .get_account_with_commitment(&programdata_address, config.commitment)?
                        .value
                    {
                        if let Ok(UpgradeableLoaderState::ProgramData { .. }) =
                            programdata_account.state()
                        {
                            let offset = UpgradeableLoaderState::size_of_programdata_metadata();
                            let program_data = &programdata_account.data[offset..];
                            let mut f = File::create(output_location)?;
                            f.write_all(program_data)?;
                            Ok(format!("Wrote program to {output_location}"))
                        } else {
                            Err(format!("Program {account_pubkey} has been closed").into())
                        }
                    } else {
                        Err(format!("Program {account_pubkey} has been closed").into())
                    }
                } else if let Ok(UpgradeableLoaderState::Buffer { .. }) = account.state() {
                    let offset = UpgradeableLoaderState::size_of_buffer_metadata();
                    let program_data = &account.data[offset..];
                    let mut f = File::create(output_location)?;
                    f.write_all(program_data)?;
                    Ok(format!("Wrote program to {output_location}"))
                } else {
                    Err(format!(
                        "{account_pubkey} is not an upgradeable loader buffer or program account"
                    )
                    .into())
                }
            } else {
                Err(format!("{account_pubkey} is not an SBF program").into())
            }
        } else {
            Err(format!("Unable to find the account {account_pubkey}").into())
        }
    } else {
        Err("No account specified".into())
    }
}

fn close(
    rpc_client: &RpcClient,
    config: &CliConfig,
    account_pubkey: &Pubkey,
    recipient_pubkey: &Pubkey,
    authority_signer: &dyn Signer,
    program_pubkey: Option<&Pubkey>,
) -> Result<(), Box<dyn std::error::Error>> {
    let blockhash = rpc_client.get_latest_blockhash()?;

    let mut tx = Transaction::new_unsigned(Message::new(
        &[bpf_loader_upgradeable::close_any(
            account_pubkey,
            recipient_pubkey,
            Some(&authority_signer.pubkey()),
            program_pubkey,
        )],
        Some(&config.signers[0].pubkey()),
    ));

    tx.try_sign(&[config.signers[0], authority_signer], blockhash)?;
    let result = rpc_client.send_and_confirm_transaction_with_spinner_and_config(
        &tx,
        config.commitment,
        RpcSendTransactionConfig {
            preflight_commitment: Some(config.commitment.commitment),
            ..RpcSendTransactionConfig::default()
        },
    );
    if let Err(err) = result {
        if let ClientErrorKind::TransactionError(TransactionError::InstructionError(
            _,
            InstructionError::InvalidInstructionData,
        )) = err.kind()
        {
            return Err("Closing a buffer account is not supported by the cluster".into());
        } else if let ClientErrorKind::TransactionError(TransactionError::InstructionError(
            _,
            InstructionError::InvalidArgument,
        )) = err.kind()
        {
            return Err("Closing a program account is not supported by the cluster".into());
        } else {
            return Err(format!("Close failed: {err}").into());
        }
    }
    Ok(())
}

fn process_close(
    rpc_client: &RpcClient,
    config: &CliConfig,
    account_pubkey: Option<Pubkey>,
    recipient_pubkey: Pubkey,
    authority_index: SignerIndex,
    use_lamports_unit: bool,
    bypass_warning: bool,
) -> ProcessResult {
    let authority_signer = config.signers[authority_index];

    if let Some(account_pubkey) = account_pubkey {
        if let Some(account) = rpc_client
            .get_account_with_commitment(&account_pubkey, config.commitment)?
            .value
        {
            match account.state() {
                Ok(UpgradeableLoaderState::Buffer { authority_address }) => {
                    if authority_address != Some(authority_signer.pubkey()) {
                        return Err(format!(
                            "Buffer account authority {:?} does not match {:?}",
                            authority_address,
                            Some(authority_signer.pubkey())
                        )
                        .into());
                    } else {
                        close(
                            rpc_client,
                            config,
                            &account_pubkey,
                            &recipient_pubkey,
                            authority_signer,
                            None,
                        )?;
                    }
                    Ok(config
                        .output_format
                        .formatted_string(&CliUpgradeableBuffers {
                            buffers: vec![CliUpgradeableBuffer {
                                address: account_pubkey.to_string(),
                                authority: authority_address
                                    .map(|pubkey| pubkey.to_string())
                                    .unwrap_or_else(|| "none".to_string()),
                                data_len: 0,
                                lamports: account.lamports,
                                use_lamports_unit,
                            }],
                            use_lamports_unit,
                        }))
                }
                Ok(UpgradeableLoaderState::Program {
                    programdata_address: programdata_pubkey,
                }) => {
                    if let Some(account) = rpc_client
                        .get_account_with_commitment(&programdata_pubkey, config.commitment)?
                        .value
                    {
                        if let Ok(UpgradeableLoaderState::ProgramData {
                            slot: _,
                            upgrade_authority_address: authority_pubkey,
                        }) = account.state()
                        {
                            if authority_pubkey != Some(authority_signer.pubkey()) {
                                Err(format!(
                                    "Program authority {:?} does not match {:?}",
                                    authority_pubkey,
                                    Some(authority_signer.pubkey())
                                )
                                .into())
                            } else {
                                if !bypass_warning {
                                    return Err(String::from(CLOSE_PROGRAM_WARNING).into());
                                }
                                close(
                                    rpc_client,
                                    config,
                                    &programdata_pubkey,
                                    &recipient_pubkey,
                                    authority_signer,
                                    Some(&account_pubkey),
                                )?;
                                Ok(config.output_format.formatted_string(
                                    &CliUpgradeableProgramClosed {
                                        program_id: account_pubkey.to_string(),
                                        lamports: account.lamports,
                                        use_lamports_unit,
                                    },
                                ))
                            }
                        } else {
                            Err(format!("Program {account_pubkey} has been closed").into())
                        }
                    } else {
                        Err(format!("Program {account_pubkey} has been closed").into())
                    }
                }
                _ => Err(format!("{account_pubkey} is not a Program or Buffer account").into()),
            }
        } else {
            Err(format!("Unable to find the account {account_pubkey}").into())
        }
    } else {
        let buffers = get_buffers(
            rpc_client,
            Some(authority_signer.pubkey()),
            use_lamports_unit,
        )?;

        let mut closed = vec![];
        for buffer in buffers.buffers.iter() {
            if close(
                rpc_client,
                config,
                &Pubkey::from_str(&buffer.address)?,
                &recipient_pubkey,
                authority_signer,
                None,
            )
            .is_ok()
            {
                closed.push(buffer.clone());
            }
        }
        Ok(config
            .output_format
            .formatted_string(&CliUpgradeableBuffers {
                buffers: closed,
                use_lamports_unit,
            }))
    }
}

fn process_extend_program(
    rpc_client: &RpcClient,
    config: &CliConfig,
    program_pubkey: Pubkey,
    additional_bytes: u32,
) -> ProcessResult {
    let payer_pubkey = config.signers[0].pubkey();

    if additional_bytes == 0 {
        return Err("Additional bytes must be greater than zero".into());
    }

    let program_account = match rpc_client
        .get_account_with_commitment(&program_pubkey, config.commitment)?
        .value
    {
        Some(program_account) => Ok(program_account),
        None => Err(format!("Unable to find program {program_pubkey}")),
    }?;

    if !bpf_loader_upgradeable::check_id(&program_account.owner) {
        return Err(format!("Account {program_pubkey} is not an upgradeable program").into());
    }

    let programdata_pubkey = match program_account.state() {
        Ok(UpgradeableLoaderState::Program {
            programdata_address: programdata_pubkey,
        }) => Ok(programdata_pubkey),
        _ => Err(format!(
            "Account {program_pubkey} is not an upgradeable program"
        )),
    }?;

    let programdata_account = match rpc_client
        .get_account_with_commitment(&programdata_pubkey, config.commitment)?
        .value
    {
        Some(programdata_account) => Ok(programdata_account),
        None => Err(format!("Program {program_pubkey} is closed")),
    }?;

    let upgrade_authority_address = match programdata_account.state() {
        Ok(UpgradeableLoaderState::ProgramData {
            slot: _,
            upgrade_authority_address,
        }) => Ok(upgrade_authority_address),
        _ => Err(format!("Program {program_pubkey} is closed")),
    }?;

    match upgrade_authority_address {
        None => Err(format!("Program {program_pubkey} is not upgradeable")),
        _ => Ok(()),
    }?;

    let blockhash = rpc_client.get_latest_blockhash()?;

    let mut tx = Transaction::new_unsigned(Message::new(
        &[bpf_loader_upgradeable::extend_program(
            &program_pubkey,
            Some(&payer_pubkey),
            additional_bytes,
        )],
        Some(&payer_pubkey),
    ));

    tx.try_sign(&[config.signers[0]], blockhash)?;
    let result = rpc_client.send_and_confirm_transaction_with_spinner_and_config(
        &tx,
        config.commitment,
        RpcSendTransactionConfig {
            preflight_commitment: Some(config.commitment.commitment),
            ..RpcSendTransactionConfig::default()
        },
    );
    if let Err(err) = result {
        if let ClientErrorKind::TransactionError(TransactionError::InstructionError(
            _,
            InstructionError::InvalidInstructionData,
        )) = err.kind()
        {
            return Err("Extending a program is not supported by the cluster".into());
        } else {
            return Err(format!("Extend program failed: {err}").into());
        }
    }

    Ok(config
        .output_format
        .formatted_string(&CliUpgradeableProgramExtended {
            program_id: program_pubkey.to_string(),
            additional_bytes,
        }))
}

pub fn calculate_max_chunk_size<F>(create_msg: &F) -> usize
where
    F: Fn(u32, Vec<u8>) -> Message,
{
    let baseline_msg = create_msg(0, Vec::new());
    let tx_size = bincode::serialized_size(&Transaction {
        signatures: vec![
            Signature::default();
            baseline_msg.header.num_required_signatures as usize
        ],
        message: baseline_msg,
    })
    .unwrap() as usize;
    // add 1 byte buffer to account for shortvec encoding
    PACKET_DATA_SIZE.saturating_sub(tx_size).saturating_sub(1)
}

#[allow(clippy::too_many_arguments)]
fn do_process_program_write_and_deploy(
    rpc_client: Arc<RpcClient>,
    config: &CliConfig,
    program_data: &[u8], // can be empty, hence we have program_len
    program_len: usize,
    program_data_max_len: usize,
    min_rent_exempt_program_data_balance: u64,
    loader_id: &Pubkey,
    fee_payer_signer: &dyn Signer,
    program_signers: Option<&[&dyn Signer]>,
    buffer_signer: Option<&dyn Signer>,
    buffer_pubkey: &Pubkey,
    buffer_authority_signer: &dyn Signer,
    allow_excessive_balance: bool,
    skip_fee_check: bool,
) -> ProcessResult {
    let blockhash = rpc_client.get_latest_blockhash()?;

    // Initialize buffer account or complete if already partially initialized
    let (initial_instructions, balance_needed, buffer_program_data) = if let Some(mut account) =
        rpc_client
            .get_account_with_commitment(buffer_pubkey, config.commitment)?
            .value
    {
        let (ixs, balance_needed) = complete_partial_program_init(
            loader_id,
            &fee_payer_signer.pubkey(),
            buffer_pubkey,
            &account,
            if loader_id == &bpf_loader_upgradeable::id() {
                UpgradeableLoaderState::size_of_buffer(program_len)
            } else {
                program_len
            },
            min_rent_exempt_program_data_balance,
            allow_excessive_balance,
        )?;
        let buffer_program_data = account
            .data
            .split_off(UpgradeableLoaderState::size_of_buffer_metadata());
        (ixs, balance_needed, buffer_program_data)
    } else if loader_id == &bpf_loader_upgradeable::id() {
        (
            bpf_loader_upgradeable::create_buffer(
                &fee_payer_signer.pubkey(),
                buffer_pubkey,
                &buffer_authority_signer.pubkey(),
                min_rent_exempt_program_data_balance,
                program_len,
            )?,
            min_rent_exempt_program_data_balance,
            vec![0; program_len],
        )
    } else {
        (
            vec![system_instruction::create_account(
                &fee_payer_signer.pubkey(),
                buffer_pubkey,
                min_rent_exempt_program_data_balance,
                program_len as u64,
                loader_id,
            )],
            min_rent_exempt_program_data_balance,
            vec![0; program_len],
        )
    };
    let initial_message = if !initial_instructions.is_empty() {
        Some(Message::new_with_blockhash(
            &initial_instructions,
            Some(&fee_payer_signer.pubkey()),
            &blockhash,
        ))
    } else {
        None
    };

    // Create and add write messages
    let create_msg = |offset: u32, bytes: Vec<u8>| {
        let instruction = if loader_id == &bpf_loader_upgradeable::id() {
            bpf_loader_upgradeable::write(
                buffer_pubkey,
                &buffer_authority_signer.pubkey(),
                offset,
                bytes,
            )
        } else {
            loader_instruction::write(buffer_pubkey, loader_id, offset, bytes)
        };
        Message::new_with_blockhash(&[instruction], Some(&fee_payer_signer.pubkey()), &blockhash)
    };

    let mut write_messages = vec![];
    let chunk_size = calculate_max_chunk_size(&create_msg);
    for (chunk, i) in program_data.chunks(chunk_size).zip(0..) {
        let offset = i * chunk_size;
        if chunk != &buffer_program_data[offset..offset + chunk.len()] {
            write_messages.push(create_msg(offset as u32, chunk.to_vec()));
        }
    }

    // Create and add final message
    let final_message = if let Some(program_signers) = program_signers {
        let message = if loader_id == &bpf_loader_upgradeable::id() {
            Message::new_with_blockhash(
                &bpf_loader_upgradeable::deploy_with_max_program_len(
                    &fee_payer_signer.pubkey(),
                    &program_signers[0].pubkey(),
                    buffer_pubkey,
                    &program_signers[1].pubkey(),
                    rpc_client.get_minimum_balance_for_rent_exemption(
                        UpgradeableLoaderState::size_of_program(),
                    )?,
                    program_data_max_len,
                )?,
                Some(&fee_payer_signer.pubkey()),
                &blockhash,
            )
        } else {
            Message::new_with_blockhash(
                &[loader_instruction::finalize(buffer_pubkey, loader_id)],
                Some(&fee_payer_signer.pubkey()),
                &blockhash,
            )
        };
        Some(message)
    } else {
        None
    };

    if !skip_fee_check {
        check_payer(
            &rpc_client,
            config,
            fee_payer_signer.pubkey(),
            balance_needed,
            &initial_message,
            &write_messages,
            &final_message,
        )?;
    }

    let final_tx_sig = send_deploy_messages(
        rpc_client,
        config,
        &initial_message,
        &write_messages,
        &final_message,
        fee_payer_signer,
        buffer_signer,
        Some(buffer_authority_signer),
        program_signers,
    )?;

    if let Some(program_signers) = program_signers {
        let program_id = CliProgramId {
            program_id: program_signers[0].pubkey().to_string(),
            signature: final_tx_sig.as_ref().map(ToString::to_string),
        };
        Ok(config.output_format.formatted_string(&program_id))
    } else {
        let buffer = CliProgramBuffer {
            buffer: buffer_pubkey.to_string(),
        };
        Ok(config.output_format.formatted_string(&buffer))
    }
}

#[allow(clippy::too_many_arguments)]
fn do_process_program_upgrade(
    rpc_client: Arc<RpcClient>,
    config: &CliConfig,
    program_data: &[u8], // can be empty, hence we have program_len
    program_len: usize,
    min_rent_exempt_program_data_balance: u64,
    fee_payer_signer: &dyn Signer,
    program_id: &Pubkey,
    upgrade_authority: &dyn Signer,
    buffer_pubkey: &Pubkey,
    buffer_signer: Option<&dyn Signer>,
    skip_fee_check: bool,
) -> ProcessResult {
    let blockhash = rpc_client.get_latest_blockhash()?;

    let (initial_message, write_messages, balance_needed) =
        if let Some(buffer_signer) = buffer_signer {
            // Check Buffer account to see if partial initialization has occurred
            let (initial_instructions, balance_needed, buffer_program_data) =
                if let Some(mut account) = rpc_client
                    .get_account_with_commitment(&buffer_signer.pubkey(), config.commitment)?
                    .value
                {
                    let (ixs, balance_needed) = complete_partial_program_init(
                        &bpf_loader_upgradeable::id(),
                        &fee_payer_signer.pubkey(),
                        &buffer_signer.pubkey(),
                        &account,
                        UpgradeableLoaderState::size_of_buffer(program_len),
                        min_rent_exempt_program_data_balance,
                        true,
                    )?;
                    let buffer_program_data = account
                        .data
                        .split_off(UpgradeableLoaderState::size_of_buffer_metadata());
                    (ixs, balance_needed, buffer_program_data)
                } else {
                    (
                        bpf_loader_upgradeable::create_buffer(
                            &fee_payer_signer.pubkey(),
                            buffer_pubkey,
                            &upgrade_authority.pubkey(),
                            min_rent_exempt_program_data_balance,
                            program_len,
                        )?,
                        min_rent_exempt_program_data_balance,
                        vec![0; program_len],
                    )
                };

            let initial_message = if !initial_instructions.is_empty() {
                Some(Message::new_with_blockhash(
                    &initial_instructions,
                    Some(&fee_payer_signer.pubkey()),
                    &blockhash,
                ))
            } else {
                None
            };

            let buffer_signer_pubkey = buffer_signer.pubkey();
            let upgrade_authority_pubkey = upgrade_authority.pubkey();
            let create_msg = |offset: u32, bytes: Vec<u8>| {
                let instruction = bpf_loader_upgradeable::write(
                    &buffer_signer_pubkey,
                    &upgrade_authority_pubkey,
                    offset,
                    bytes,
                );
                Message::new_with_blockhash(
                    &[instruction],
                    Some(&fee_payer_signer.pubkey()),
                    &blockhash,
                )
            };

            // Create and add write messages
            let mut write_messages = vec![];
            let chunk_size = calculate_max_chunk_size(&create_msg);
            for (chunk, i) in program_data.chunks(chunk_size).zip(0..) {
                let offset = i * chunk_size;
                if chunk != &buffer_program_data[offset..offset + chunk.len()] {
                    write_messages.push(create_msg(offset as u32, chunk.to_vec()));
                }
            }

            (initial_message, write_messages, balance_needed)
        } else {
            (None, vec![], 0)
        };

    // Create and add final message
    let final_message = Message::new_with_blockhash(
        &[bpf_loader_upgradeable::upgrade(
            program_id,
            buffer_pubkey,
            &upgrade_authority.pubkey(),
            &fee_payer_signer.pubkey(),
        )],
        Some(&fee_payer_signer.pubkey()),
        &blockhash,
    );
    let final_message = Some(final_message);

    if !skip_fee_check {
        check_payer(
            &rpc_client,
            config,
            fee_payer_signer.pubkey(),
            balance_needed,
            &initial_message,
            &write_messages,
            &final_message,
        )?;
    }

    let final_tx_sig = send_deploy_messages(
        rpc_client,
        config,
        &initial_message,
        &write_messages,
        &final_message,
        fee_payer_signer,
        buffer_signer,
        Some(upgrade_authority),
        Some(&[upgrade_authority]),
    )?;

    let program_id = CliProgramId {
        program_id: program_id.to_string(),
        signature: final_tx_sig.as_ref().map(ToString::to_string),
    };
    Ok(config.output_format.formatted_string(&program_id))
}

fn read_and_verify_elf(program_location: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut file = File::open(program_location)
        .map_err(|err| format!("Unable to open program file: {err}"))?;
    let mut program_data = Vec::new();
    file.read_to_end(&mut program_data)
        .map_err(|err| format!("Unable to read program file: {err}"))?;

    // Verify the program
    let program_runtime_environment = create_program_runtime_environment_v1(
        &FeatureSet::all_enabled(),
        &ComputeBudget::default(),
        true,
        false,
    )
    .unwrap();
    let executable =
        Executable::<InvokeContext>::from_elf(&program_data, Arc::new(program_runtime_environment))
            .map_err(|err| format!("ELF error: {err}"))?;

    executable
        .verify::<RequisiteVerifier>()
        .map_err(|err| format!("ELF error: {err}"))?;

    Ok(program_data)
}

fn complete_partial_program_init(
    loader_id: &Pubkey,
    payer_pubkey: &Pubkey,
    elf_pubkey: &Pubkey,
    account: &Account,
    account_data_len: usize,
    minimum_balance: u64,
    allow_excessive_balance: bool,
) -> Result<(Vec<Instruction>, u64), Box<dyn std::error::Error>> {
    let mut instructions: Vec<Instruction> = vec![];
    let mut balance_needed = 0;
    if account.executable {
        return Err("Buffer account is already executable".into());
    }
    if account.owner != *loader_id && !system_program::check_id(&account.owner) {
        return Err("Buffer account passed is already in use by another program".into());
    }
    if !account.data.is_empty() && account.data.len() < account_data_len {
        return Err(
            "Buffer account passed is not large enough, may have been for a different deploy?"
                .into(),
        );
    }

    if account.data.is_empty() && system_program::check_id(&account.owner) {
        instructions.push(system_instruction::allocate(
            elf_pubkey,
            account_data_len as u64,
        ));
        instructions.push(system_instruction::assign(elf_pubkey, loader_id));
        if account.lamports < minimum_balance {
            let balance = minimum_balance - account.lamports;
            instructions.push(system_instruction::transfer(
                payer_pubkey,
                elf_pubkey,
                balance,
            ));
            balance_needed = balance;
        } else if account.lamports > minimum_balance
            && system_program::check_id(&account.owner)
            && !allow_excessive_balance
        {
            return Err(format!(
                "Buffer account has a balance: {:?}; it may already be in use",
                TACHYON(account.lamports)
            )
            .into());
        }
    }
    Ok((instructions, balance_needed))
}

fn check_payer(
    rpc_client: &RpcClient,
    config: &CliConfig,
    fee_payer_pubkey: Pubkey,
    balance_needed: u64,
    initial_message: &Option<Message>,
    write_messages: &[Message],
    final_message: &Option<Message>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut fee = 0;
    if let Some(message) = initial_message {
        fee += rpc_client.get_fee_for_message(message)?;
    }
    if !write_messages.is_empty() {
        // Assume all write messages cost the same
        if let Some(message) = write_messages.first() {
            fee += rpc_client.get_fee_for_message(message)? * (write_messages.len() as u64);
        }
    }
    if let Some(message) = final_message {
        fee += rpc_client.get_fee_for_message(message)?;
    }
    check_account_for_spend_and_fee_with_commitment(
        rpc_client,
        &fee_payer_pubkey,
        balance_needed,
        fee,
        config.commitment,
    )?;
    Ok(())
}

fn send_deploy_messages(
    rpc_client: Arc<RpcClient>,
    config: &CliConfig,
    initial_message: &Option<Message>,
    write_messages: &[Message],
    final_message: &Option<Message>,
    fee_payer_signer: &dyn Signer,
    initial_signer: Option<&dyn Signer>,
    write_signer: Option<&dyn Signer>,
    final_signers: Option<&[&dyn Signer]>,
) -> Result<Option<Signature>, Box<dyn std::error::Error>> {
    if let Some(message) = initial_message {
        if let Some(initial_signer) = initial_signer {
            trace!("Preparing the required accounts");
            let blockhash = rpc_client.get_latest_blockhash()?;

            let mut initial_transaction = Transaction::new_unsigned(message.clone());
            // Most of the initial_transaction combinations require both the fee-payer and new program
            // account to sign the transaction. One (transfer) only requires the fee-payer signature.
            // This check is to ensure signing does not fail on a KeypairPubkeyMismatch error from an
            // extraneous signature.
            if message.header.num_required_signatures == 2 {
                initial_transaction.try_sign(&[fee_payer_signer, initial_signer], blockhash)?;
            } else {
                initial_transaction.try_sign(&[fee_payer_signer], blockhash)?;
            }
            let result = rpc_client.send_and_confirm_transaction_with_spinner(&initial_transaction);
            log_instruction_custom_error::<SystemError>(result, config)
                .map_err(|err| format!("Account allocation failed: {err}"))?;
        } else {
            return Err("Buffer account not created yet, must provide a key pair".into());
        }
    }

    if !write_messages.is_empty() {
        if let Some(write_signer) = write_signer {
            trace!("Writing program data");
            let connection_cache = if config.use_quic {
                ConnectionCache::new_quic("connection_cache_cli_program_quic", 1)
            } else {
                ConnectionCache::with_udp("connection_cache_cli_program_udp", 1)
            };
            let transaction_errors = match connection_cache {
                ConnectionCache::Udp(cache) => TpuClient::new_with_connection_cache(
                    rpc_client.clone(),
                    &config.websocket_url,
                    TpuClientConfig::default(),
                    cache,
                )?
                .send_and_confirm_messages_with_spinner(
                    write_messages,
                    &[fee_payer_signer, write_signer],
                ),
                ConnectionCache::Quic(cache) => {
                    let tpu_client_fut = Alembic_client::nonblocking::tpu_client::TpuClient::new_with_connection_cache(
                        rpc_client.get_inner_client().clone(),
                        config.websocket_url.as_str(),
                        Alembic_client::tpu_client::TpuClientConfig::default(),
                        cache,
                    );
                    let tpu_client = rpc_client
                        .runtime()
                        .block_on(tpu_client_fut)
                        .expect("Should return a valid tpu client");

                    send_and_confirm_transactions_in_parallel_blocking(
                        rpc_client.clone(),
                        Some(tpu_client),
                        write_messages,
                        &[fee_payer_signer, write_signer],
                        SendAndConfirmConfig {
                            resign_txs_count: Some(5),
                            with_spinner: true,
                        },
                    )
                },
            }
            .map_err(|err| format!("Data writes to account failed: {err}"))?
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();

            if !transaction_errors.is_empty() {
                for transaction_error in &transaction_errors {
                    error!("{:?}", transaction_error);
                }
                return Err(
                    format!("{} write transactions failed", transaction_errors.len()).into(),
                );
            }
        }
    }

    if let Some(message) = final_message {
        if let Some(final_signers) = final_signers {
            trace!("Deploying program");
            let blockhash = rpc_client.get_latest_blockhash()?;

            let mut final_tx = Transaction::new_unsigned(message.clone());
            let mut signers = final_signers.to_vec();
            signers.push(fee_payer_signer);
            final_tx.try_sign(&signers, blockhash)?;
            return Ok(Some(
                rpc_client
                    .send_and_confirm_transaction_with_spinner_and_config(
                        &final_tx,
                        config.commitment,
                        RpcSendTransactionConfig {
                            preflight_commitment: Some(config.commitment.commitment),
                            ..RpcSendTransactionConfig::default()
                        },
                    )
                    .map_err(|e| format!("Deploying program failed: {e}"))?,
            ));
        }
    }

    Ok(None)
}

fn create_ephemeral_keypair(
) -> Result<(usize, bip39::Mnemonic, Keypair), Box<dyn std::error::Error>> {
    const WORDS: usize = 12;
    let mnemonic = Mnemonic::new(MnemonicType::for_word_count(WORDS)?, Language::English);
    let seed = Seed::new(&mnemonic, "");
    let new_keypair = keypair_from_seed(seed.as_bytes())?;

    Ok((WORDS, mnemonic, new_keypair))
}

fn report_ephemeral_mnemonic(words: usize, mnemonic: bip39::Mnemonic) {
    let phrase: &str = mnemonic.phrase();
    let divider = String::from_utf8(vec![b'='; phrase.len()]).unwrap();
    eprintln!("{divider}\nRecover the intermediate account's ephemeral keypair file with");
    eprintln!("`Alembic-keygen recover` and the following {words}-word seed phrase:");
    eprintln!("{divider}\n{phrase}\n{divider}");
    eprintln!("To resume a deploy, pass the recovered keypair as the");
    eprintln!("[BUFFER_SIGNER] to `Alembic program deploy` or `Alembic program write-buffer'.");
    eprintln!("Or to recover the account's lamports, pass it as the");
    eprintln!("[BUFFER_ACCOUNT_ADDRESS] argument to `Alembic program close`.\n{divider}");
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            clap_app::get_clap_app,
            cli::{parse_command, process_command},
        },
        serde_json::Value,
        Alembic_cli_output::OutputFormat,
        Alembic_sdk::{hash::Hash, signature::write_keypair_file},
    };

    fn make_tmp_path(name: &str) -> String {
        let out_dir = std::env::var("FARF_DIR").unwrap_or_else(|_| "farf".to_string());
        let keypair = Keypair::new();

        let path = format!("{}/tmp/{}-{}", out_dir, name, keypair.pubkey());

        // whack any possible collision
        let _ignored = std::fs::remove_dir_all(&path);
        // whack any possible collision
        let _ignored = std::fs::remove_file(&path);

        path
    }

    #[test]
    #[allow(clippy::cognitive_complexity)]
    fn test_cli_parse_deploy() {
        let test_commands = get_clap_app("test", "desc", "version");

        let default_keypair = Keypair::new();
        let keypair_file = make_tmp_path("keypair_file");
        write_keypair_file(&default_keypair, &keypair_file).unwrap();
        let default_signer = DefaultSigner::new("", &keypair_file);

        let test_command = test_commands.clone().get_matches_from(vec![
            "test",
            "program",
            "deploy",
            "/Users/test/program.so",
        ]);
        assert_eq!(
            parse_command(&test_command, &default_signer, &mut None).unwrap(),
            CliCommandInfo {
                command: CliCommand::Program(ProgramCliCommand::Deploy {
                    program_location: Some("/Users/test/program.so".to_string()),
                    fee_payer_signer_index: 0,
                    buffer_signer_index: None,
                    buffer_pubkey: None,
                    program_signer_index: None,
                    program_pubkey: None,
                    upgrade_authority_signer_index: 0,
                    is_final: false,
                    max_len: None,
                    allow_excessive_balance: false,
                    skip_fee_check: false,
                }),
                signers: vec![Box::new(read_keypair_file(&keypair_file).unwrap())],
            }
        );

        let test_command = test_commands.clone().get_matches_from(vec![
            "test",
            "program",
            "deploy",
            "/Users/test/program.so",
            "--max-len",
            "42",
        ]);
        assert_eq!(
            parse_command(&test_command, &default_signer, &mut None).unwrap(),
            CliCommandInfo {
                command: CliCommand::Program(ProgramCliCommand::Deploy {
                    program_location: Some("/Users/test/program.so".to_string()),
                    fee_payer_signer_index: 0,
                    buffer_signer_index: None,
                    buffer_pubkey: None,
                    program_signer_index: None,
                    program_pubkey: None,
                    upgrade_authority_signer_index: 0,
                    is_final: false,
                    max_len: Some(42),
                    allow_excessive_balance: false,
                    skip_fee_check: false,
                }),
                signers: vec![Box::new(read_keypair_file(&keypair_file).unwrap())],
            }
        );

        let buffer_keypair = Keypair::new();
        let buffer_keypair_file = make_tmp_path("buffer_keypair_file");
        write_keypair_file(&buffer_keypair, &buffer_keypair_file).unwrap();
        let test_command = test_commands.clone().get_matches_from(vec![
            "test",
            "program",
            "deploy",
            "--buffer",
            &buffer_keypair_file,
        ]);
        assert_eq!(
            parse_command(&test_command, &default_signer, &mut None).unwrap(),
            CliCommandInfo {
                command: CliCommand::Program(ProgramCliCommand::Deploy {
                    program_location: None,
                    fee_payer_signer_index: 0,
                    buffer_signer_index: Some(1),
                    buffer_pubkey: Some(buffer_keypair.pubkey()),
                    program_signer_index: None,
                    program_pubkey: None,
                    upgrade_authority_signer_index: 0,
                    is_final: false,
                    max_len: None,
                    allow_excessive_balance: false,
                    skip_fee_check: false,
                }),
                signers: vec![
                    Box::new(read_keypair_file(&keypair_file).unwrap()),
                    Box::new(read_keypair_file(&buffer_keypair_file).unwrap()),
                ],
            }
        );

        let program_pubkey = Pubkey::new_unique();
        let test = test_commands.clone().get_matches_from(vec![
            "test",
            "program",
            "deploy",
            "/Users/test/program.so",
            "--program-id",
            &program_pubkey.to_string(),
        ]);
        assert_eq!(
            parse_command(&test, &default_signer, &mut None).unwrap(),
            CliCommandInfo {
                command: CliCommand::Program(ProgramCliCommand::Deploy {
                    program_location: Some("/Users/test/program.so".to_string()),
                    fee_payer_signer_index: 0,
                    buffer_signer_index: None,
                    buffer_pubkey: None,
                    program_signer_index: None,
                    program_pubkey: Some(program_pubkey),
                    upgrade_authority_signer_index: 0,
                    is_final: false,
                    max_len: None,
                    allow_excessive_balance: false,
                    skip_fee_check: false,
                }),
                signers: vec![Box::new(read_keypair_file(&keypair_file).unwrap())],
            }
        );

        let program_keypair = Keypair::new();
        let program_keypair_file = make_tmp_path("program_keypair_file");
        write_keypair_file(&program_keypair, &program_keypair_file).unwrap();
        let test = test_commands.clone().get_matches_from(vec![
            "test",
            "program",
            "deploy",
            "/Users/test/program.so",
            "--program-id",
            &program_keypair_file,
        ]);
        assert_eq!(
            parse_command(&test, &default_signer, &mut None).unwrap(),
            CliCommandInfo {
                command: CliCommand::Program(ProgramCliCommand::Deploy {
                    program_location: Some("/Users/test/program.so".to_string()),
                    fee_payer_signer_index: 0,
                    buffer_signer_index: None,
                    buffer_pubkey: None,
                    program_signer_index: Some(1),
                    program_pubkey: Some(program_keypair.pubkey()),
                    upgrade_authority_signer_index: 0,
                    is_final: false,
                    max_len: None,
                    allow_excessive_balance: false,
                    skip_fee_check: false,
                }),
                signers: vec![
                    Box::new(read_keypair_file(&keypair_file).unwrap()),
                    Box::new(read_keypair_file(&program_keypair_file).unwrap()),
                ],
            }
        );

        let authority_keypair = Keypair::new();
        let authority_keypair_file = make_tmp_path("authority_keypair_file");
        write_keypair_file(&authority_keypair, &authority_keypair_file).unwrap();
        let test_command = test_commands.clone().get_matches_from(vec![
            "test",
            "program",
            "deploy",
            "/Users/test/program.so",
            "--upgrade-authority",
            &authority_keypair_file,
        ]);
        assert_eq!(
            parse_command(&test_command, &default_signer, &mut None).unwrap(),
            CliCommandInfo {
                command: CliCommand::Program(ProgramCliCommand::Deploy {
                    program_location: Some("/Users/test/program.so".to_string()),
                    fee_payer_signer_index: 0,
                    buffer_signer_index: None,
                    buffer_pubkey: None,
                    program_signer_index: None,
                    program_pubkey: None,
                    upgrade_authority_signer_index: 1,
                    is_final: false,
                    max_len: None,
                    allow_excessive_balance: false,
                    skip_fee_check: false,
                }),
                signers: vec![
                    Box::new(read_keypair_file(&keypair_file).unwrap()),
                    Box::new(read_keypair_file(&authority_keypair_file).unwrap()),
                ],
            }
        );

        let test_command = test_commands.clone().get_matches_from(vec![
            "test",
            "program",
            "deploy",
            "/Users/test/program.so",
            "--final",
        ]);
        assert_eq!(
            parse_command(&test_command, &default_signer, &mut None).unwrap(),
            CliCommandInfo {
                command: CliCommand::Program(ProgramCliCommand::Deploy {
                    program_location: Some("/Users/test/program.so".to_string()),
                    fee_payer_signer_index: 0,
                    buffer_signer_index: None,
                    buffer_pubkey: None,
                    program_signer_index: None,
                    program_pubkey: None,
                    upgrade_authority_signer_index: 0,
                    is_final: true,
                    max_len: None,
                    skip_fee_check: false,
                    allow_excessive_balance: false,
                }),
                signers: vec![Box::new(read_keypair_file(&keypair_file).unwrap())],
            }
        );
    }

    #[test]
    #[allow(clippy::cognitive_complexity)]
    fn test_cli_parse_write_buffer() {
        let test_commands = get_clap_app("test", "desc", "version");

        let default_keypair = Keypair::new();
        let keypair_file = make_tmp_path("keypair_file");
        write_keypair_file(&default_keypair, &keypair_file).unwrap();
        let default_signer = DefaultSigner::new("", &keypair_file);

        // defaults
        let test_command = test_commands.clone().get_matches_from(vec![
            "test",
            "program",
            "write-buffer",
            "/Users/test/program.so",
        ]);
        assert_eq!(
            parse_command(&test_command, &default_signer, &mut None).unwrap(),
            CliCommandInfo {
                command: CliCommand::Program(ProgramCliCommand::WriteBuffer {
                    program_location: "/Users/test/program.so".to_string(),
                    fee_payer_signer_index: 0,
                    buffer_signer_index: None,
                    buffer_pubkey: None,
                    buffer_authority_signer_index: 0,
                    max_len: None,
                    skip_fee_check: false,
                }),
                signers: vec![Box::new(read_keypair_file(&keypair_file).unwrap())],
            }
        );

        // specify max len
        let test_command = test_commands.clone().get_matches_from(vec![
            "test",
            "program",
            "write-buffer",
            "/Users/test/program.so",
            "--max-len",
            "42",
        ]);
        assert_eq!(
            parse_command(&test_command, &default_signer, &mut None).unwrap(),
            CliCommandInfo {
                command: CliCommand::Program(ProgramCliCommand::WriteBuffer {
                    program_location: "/Users/test/program.so".to_string(),
                    fee_payer_signer_index: 0,
                    buffer_signer_index: None,
                    buffer_pubkey: None,
                    buffer_authority_signer_index: 0,
                    max_len: Some(42),
                    skip_fee_check: false,
                }),
                signers: vec![Box::new(read_keypair_file(&keypair_file).unwrap())],
            }
        );

        // specify buffer
        let buffer_keypair = Keypair::new();
        let buffer_keypair_file = make_tmp_path("buffer_keypair_file");
        write_keypair_file(&buffer_keypair, &buffer_keypair_file).unwrap();
        let test_command = test_commands.clone().get_matches_from(vec![
            "test",
            "program",
            "write-buffer",
            "/Users/test/program.so",
            "--buffer",
            &buffer_keypair_file,
        ]);
        assert_eq!(
            parse_command(&test_command, &default_signer, &mut None).unwrap(),
            CliCommandInfo {
                command: CliCommand::Program(ProgramCliCommand::WriteBuffer {
                    program_location: "/Users/test/program.so".to_string(),
                    fee_payer_signer_index: 0,
                    buffer_signer_index: Some(1),
                    buffer_pubkey: Some(buffer_keypair.pubkey()),
                    buffer_authority_signer_index: 0,
                    max_len: None,
                    skip_fee_check: false,
                }),
                signers: vec![
                    Box::new(read_keypair_file(&keypair_file).unwrap()),
                    Box::new(read_keypair_file(&buffer_keypair_file).unwrap()),
                ],
            }
        );

        // specify authority
        let authority_keypair = Keypair::new();
        let authority_keypair_file = make_tmp_path("authority_keypair_file");
        write_keypair_file(&authority_keypair, &authority_keypair_file).unwrap();
        let test_command = test_commands.clone().get_matches_from(vec![
            "test",
            "program",
            "write-buffer",
            "/Users/test/program.so",
            "--buffer-authority",
            &authority_keypair_file,
        ]);
        assert_eq!(
            parse_command(&test_command, &default_signer, &mut None).unwrap(),
            CliCommandInfo {
                command: CliCommand::Program(ProgramCliCommand::WriteBuffer {
                    program_location: "/Users/test/program.so".to_string(),
                    fee_payer_signer_index: 0,
                    buffer_signer_index: None,
                    buffer_pubkey: None,
                    buffer_authority_signer_index: 1,
                    max_len: None,
                    skip_fee_check: false,
                }),
                signers: vec![
                    Box::new(read_keypair_file(&keypair_file).unwrap()),
                    Box::new(read_keypair_file(&authority_keypair_file).unwrap()),
                ],
            }
        );

        // specify both buffer and authority
        let buffer_keypair = Keypair::new();
        let buffer_keypair_file = make_tmp_path("buffer_keypair_file");
        write_keypair_file(&buffer_keypair, &buffer_keypair_file).unwrap();
        let authority_keypair = Keypair::new();
        let authority_keypair_file = make_tmp_path("authority_keypair_file");
        write_keypair_file(&authority_keypair, &authority_keypair_file).unwrap();
        let test_command = test_commands.clone().get_matches_from(vec![
            "test",
            "program",
            "write-buffer",
            "/Users/test/program.so",
            "--buffer",
            &buffer_keypair_file,
            "--buffer-authority",
            &authority_keypair_file,
        ]);
        assert_eq!(
            parse_command(&test_command, &default_signer, &mut None).unwrap(),
            CliCommandInfo {
                command: CliCommand::Program(ProgramCliCommand::WriteBuffer {
                    program_location: "/Users/test/program.so".to_string(),
                    fee_payer_signer_index: 0,
                    buffer_signer_index: Some(1),
                    buffer_pubkey: Some(buffer_keypair.pubkey()),
                    buffer_authority_signer_index: 2,
                    max_len: None,
                    skip_fee_check: false,
                }),
                signers: vec![
                    Box::new(read_keypair_file(&keypair_file).unwrap()),
                    Box::new(read_keypair_file(&buffer_keypair_file).unwrap()),
                    Box::new(read_keypair_file(&authority_keypair_file).unwrap()),
                ],
            }
        );
    }

    #[test]
    #[allow(clippy::cognitive_complexity)]
    fn test_cli_parse_set_upgrade_authority() {
        let test_commands = get_clap_app("test", "desc", "version");

        let default_keypair = Keypair::new();
        let keypair_file = make_tmp_path("keypair_file");
        write_keypair_file(&default_keypair, &keypair_file).unwrap();
        let default_signer = DefaultSigner::new("", &keypair_file);

        let program_pubkey = Pubkey::new_unique();
        let new_authority_pubkey = Pubkey::new_unique();
        let blockhash = Hash::new_unique();

        let test_command = test_commands.clone().get_matches_from(vec![
            "test",
            "program",
            "set-upgrade-authority",
            &program_pubkey.to_string(),
            "--new-upgrade-authority",
            &new_authority_pubkey.to_string(),
            "--skip-new-upgrade-authority-signer-check",
            "--sign-only",
            "--dump-transaction-message",
            "--blockhash",
            blockhash.to_string().as_str(),
        ]);
        assert_eq!(
            parse_command(&test_command, &default_signer, &mut None).unwrap(),
            CliCommandInfo {
                command: CliCommand::Program(ProgramCliCommand::SetUpgradeAuthority {
                    program_pubkey,
                    upgrade_authority_index: Some(0),
                    new_upgrade_authority: Some(new_authority_pubkey),
                    sign_only: true,
                    dump_transaction_message: true,
                    blockhash_query: BlockhashQuery::new(Some(blockhash), true, None),
                }),
                signers: vec![Box::new(read_keypair_file(&keypair_file).unwrap())],
            }
        );

        let program_pubkey = Pubkey::new_unique();
        let new_authority_pubkey = Keypair::new();
        let new_authority_pubkey_file = make_tmp_path("authority_keypair_file");
        write_keypair_file(&new_authority_pubkey, &new_authority_pubkey_file).unwrap();
        let test_command = test_commands.clone().get_matches_from(vec![
            "test",
            "program",
            "set-upgrade-authority",
            &program_pubkey.to_string(),
            "--new-upgrade-authority",
            &new_authority_pubkey_file,
            "--skip-new-upgrade-authority-signer-check",
        ]);
        assert_eq!(
            parse_command(&test_command, &default_signer, &mut None).unwrap(),
            CliCommandInfo {
                command: CliCommand::Program(ProgramCliCommand::SetUpgradeAuthority {
                    program_pubkey,
                    upgrade_authority_index: Some(0),
                    new_upgrade_authority: Some(new_authority_pubkey.pubkey()),
                    sign_only: false,
                    dump_transaction_message: false,
                    blockhash_query: BlockhashQuery::default(),
                }),
                signers: vec![Box::new(read_keypair_file(&keypair_file).unwrap())],
            }
        );

        let blockhash = Hash::new_unique();
        let program_pubkey = Pubkey::new_unique();
        let new_authority_pubkey = Keypair::new();
        let new_authority_pubkey_file = make_tmp_path("authority_keypair_file");
        write_keypair_file(&new_authority_pubkey, &new_authority_pubkey_file).unwrap();
        let test_command = test_commands.clone().get_matches_from(vec![
            "test",
            "program",
            "set-upgrade-authority",
            &program_pubkey.to_string(),
            "--new-upgrade-authority",
            &new_authority_pubkey_file,
            "--sign-only",
            "--dump-transaction-message",
            "--blockhash",
            blockhash.to_string().as_str(),
        ]);
        assert_eq!(
            parse_command(&test_command, &default_signer, &mut None).unwrap(),
            CliCommandInfo {
                command: CliCommand::Program(ProgramCliCommand::SetUpgradeAuthorityChecked {
                    program_pubkey,
                    upgrade_authority_index: 0,
                    new_upgrade_authority_index: 1,
                    sign_only: true,
                    dump_transaction_message: true,
                    blockhash_query: BlockhashQuery::new(Some(blockhash), true, None),
                }),
                signers: vec![
                    Box::new(read_keypair_file(&keypair_file).unwrap()),
                    Box::new(read_keypair_file(&new_authority_pubkey_file).unwrap()),
                ],
            }
        );

        let program_pubkey = Pubkey::new_unique();
        let new_authority_pubkey = Keypair::new();
        let new_authority_pubkey_file = make_tmp_path("authority_keypair_file");
        write_keypair_file(&new_authority_pubkey, new_authority_pubkey_file).unwrap();
        let test_command = test_commands.clone().get_matches_from(vec![
            "test",
            "program",
            "set-upgrade-authority",
            &program_pubkey.to_string(),
            "--final",
        ]);
        assert_eq!(
            parse_command(&test_command, &default_signer, &mut None).unwrap(),
            CliCommandInfo {
                command: CliCommand::Program(ProgramCliCommand::SetUpgradeAuthority {
                    program_pubkey,
                    upgrade_authority_index: Some(0),
                    new_upgrade_authority: None,
                    sign_only: false,
                    dump_transaction_message: false,
                    blockhash_query: BlockhashQuery::default(),
                }),
                signers: vec![Box::new(read_keypair_file(&keypair_file).unwrap())],
            }
        );

        let program_pubkey = Pubkey::new_unique();
        let authority = Keypair::new();
        let authority_keypair_file = make_tmp_path("authority_keypair_file");
        write_keypair_file(&authority, &authority_keypair_file).unwrap();
        let test_command = test_commands.clone().get_matches_from(vec![
            "test",
            "program",
            "set-upgrade-authority",
            &program_pubkey.to_string(),
            "--upgrade-authority",
            &authority_keypair_file,
            "--final",
        ]);
        assert_eq!(
            parse_command(&test_command, &default_signer, &mut None).unwrap(),
            CliCommandInfo {
                command: CliCommand::Program(ProgramCliCommand::SetUpgradeAuthority {
                    program_pubkey,
                    upgrade_authority_index: Some(1),
                    new_upgrade_authority: None,
                    sign_only: false,
                    dump_transaction_message: false,
                    blockhash_query: BlockhashQuery::default(),
                }),
                signers: vec![
                    Box::new(read_keypair_file(&keypair_file).unwrap()),
                    Box::new(read_keypair_file(&authority_keypair_file).unwrap()),
                ],
            }
        );
    }

    #[test]
    #[allow(clippy::cognitive_complexity)]
    fn test_cli_parse_set_buffer_authority() {
        let test_commands = get_clap_app("test", "desc", "version");

        let default_keypair = Keypair::new();
        let keypair_file = make_tmp_path("keypair_file");
        write_keypair_file(&default_keypair, &keypair_file).unwrap();
        let default_signer = DefaultSigner::new("", &keypair_file);

        let buffer_pubkey = Pubkey::new_unique();
        let new_authority_pubkey = Pubkey::new_unique();
        let test_command = test_commands.clone().get_matches_from(vec![
            "test",
            "program",
            "set-buffer-authority",
            &buffer_pubkey.to_string(),
            "--new-buffer-authority",
            &new_authority_pubkey.to_string(),
        ]);
        assert_eq!(
            parse_command(&test_command, &default_signer, &mut None).unwrap(),
            CliCommandInfo {
                command: CliCommand::Program(ProgramCliCommand::SetBufferAuthority {
                    buffer_pubkey,
                    buffer_authority_index: Some(0),
                    new_buffer_authority: new_authority_pubkey,
                }),
                signers: vec![Box::new(read_keypair_file(&keypair_file).unwrap())],
            }
        );

        let buffer_pubkey = Pubkey::new_unique();
        let new_authority_keypair = Keypair::new();
        let new_authority_keypair_file = make_tmp_path("authority_keypair_file");
        write_keypair_file(&new_authority_keypair, &new_authority_keypair_file).unwrap();
        let test_command = test_commands.clone().get_matches_from(vec![
            "test",
            "program",
            "set-buffer-authority",
            &buffer_pubkey.to_string(),
            "--new-buffer-authority",
            &new_authority_keypair_file,
        ]);
        assert_eq!(
            parse_command(&test_command, &default_signer, &mut None).unwrap(),
            CliCommandInfo {
                command: CliCommand::Program(ProgramCliCommand::SetBufferAuthority {
                    buffer_pubkey,
                    buffer_authority_index: Some(0),
                    new_buffer_authority: new_authority_keypair.pubkey(),
                }),
                signers: vec![Box::new(read_keypair_file(&keypair_file).unwrap())],
            }
        );
    }

    #[test]
    #[allow(clippy::cognitive_complexity)]
    fn test_cli_parse_show() {
        let test_commands = get_clap_app("test", "desc", "version");

        let default_keypair = Keypair::new();
        let keypair_file = make_tmp_path("keypair_file");
        write_keypair_file(&default_keypair, &keypair_file).unwrap();
        let default_signer = DefaultSigner::new("", &keypair_file);

        // defaults
        let buffer_pubkey = Pubkey::new_unique();
        let authority_keypair = Keypair::new();
        let authority_keypair_file = make_tmp_path("authority_keypair_file");
        write_keypair_file(&authority_keypair, &authority_keypair_file).unwrap();

        let test_command = test_commands.clone().get_matches_from(vec![
            "test",
            "program",
            "show",
            &buffer_pubkey.to_string(),
        ]);
        assert_eq!(
            parse_command(&test_command, &default_signer, &mut None).unwrap(),
            CliCommandInfo {
                command: CliCommand::Program(ProgramCliCommand::Show {
                    account_pubkey: Some(buffer_pubkey),
                    authority_pubkey: default_keypair.pubkey(),
                    get_programs: false,
                    get_buffers: false,
                    all: false,
                    use_lamports_unit: false,
                }),
                signers: vec![],
            }
        );

        let test_command = test_commands.clone().get_matches_from(vec![
            "test",
            "program",
            "show",
            "--programs",
            "--all",
            "--lamports",
        ]);
        assert_eq!(
            parse_command(&test_command, &default_signer, &mut None).unwrap(),
            CliCommandInfo {
                command: CliCommand::Program(ProgramCliCommand::Show {
                    account_pubkey: None,
                    authority_pubkey: default_keypair.pubkey(),
                    get_programs: true,
                    get_buffers: false,
                    all: true,
                    use_lamports_unit: true,
                }),
                signers: vec![],
            }
        );

        let test_command = test_commands.clone().get_matches_from(vec![
            "test",
            "program",
            "show",
            "--buffers",
            "--all",
            "--lamports",
        ]);
        assert_eq!(
            parse_command(&test_command, &default_signer, &mut None).unwrap(),
            CliCommandInfo {
                command: CliCommand::Program(ProgramCliCommand::Show {
                    account_pubkey: None,
                    authority_pubkey: default_keypair.pubkey(),
                    get_programs: false,
                    get_buffers: true,
                    all: true,
                    use_lamports_unit: true,
                }),
                signers: vec![],
            }
        );

        let test_command = test_commands.clone().get_matches_from(vec![
            "test",
            "program",
            "show",
            "--buffers",
            "--buffer-authority",
            &authority_keypair.pubkey().to_string(),
        ]);
        assert_eq!(
            parse_command(&test_command, &default_signer, &mut None).unwrap(),
            CliCommandInfo {
                command: CliCommand::Program(ProgramCliCommand::Show {
                    account_pubkey: None,
                    authority_pubkey: authority_keypair.pubkey(),
                    get_programs: false,
                    get_buffers: true,
                    all: false,
                    use_lamports_unit: false,
                }),
                signers: vec![],
            }
        );

        let test_command = test_commands.clone().get_matches_from(vec![
            "test",
            "program",
            "show",
            "--buffers",
            "--buffer-authority",
            &authority_keypair_file,
        ]);
        assert_eq!(
            parse_command(&test_command, &default_signer, &mut None).unwrap(),
            CliCommandInfo {
                command: CliCommand::Program(ProgramCliCommand::Show {
                    account_pubkey: None,
                    authority_pubkey: authority_keypair.pubkey(),
                    get_programs: false,
                    get_buffers: true,
                    all: false,
                    use_lamports_unit: false,
                }),
                signers: vec![],
            }
        );
    }

    #[test]
    #[allow(clippy::cognitive_complexity)]
    fn test_cli_parse_close() {
        let test_commands = get_clap_app("test", "desc", "version");

        let default_keypair = Keypair::new();
        let keypair_file = make_tmp_path("keypair_file");
        write_keypair_file(&default_keypair, &keypair_file).unwrap();
        let default_signer = DefaultSigner::new("", &keypair_file);

        // defaults
        let buffer_pubkey = Pubkey::new_unique();
        let recipient_pubkey = Pubkey::new_unique();
        let authority_keypair = Keypair::new();
        let authority_keypair_file = make_tmp_path("authority_keypair_file");

        let test_command = test_commands.clone().get_matches_from(vec![
            "test",
            "program",
            "close",
            &buffer_pubkey.to_string(),
        ]);
        assert_eq!(
            parse_command(&test_command, &default_signer, &mut None).unwrap(),
            CliCommandInfo {
                command: CliCommand::Program(ProgramCliCommand::Close {
                    account_pubkey: Some(buffer_pubkey),
                    recipient_pubkey: default_keypair.pubkey(),
                    authority_index: 0,
                    use_lamports_unit: false,
                    bypass_warning: false,
                }),
                signers: vec![Box::new(read_keypair_file(&keypair_file).unwrap())],
            }
        );

        // with bypass-warning
        write_keypair_file(&authority_keypair, &authority_keypair_file).unwrap();
        let test_command = test_commands.clone().get_matches_from(vec![
            "test",
            "program",
            "close",
            &buffer_pubkey.to_string(),
            "--bypass-warning",
        ]);
        assert_eq!(
            parse_command(&test_command, &default_signer, &mut None).unwrap(),
            CliCommandInfo {
                command: CliCommand::Program(ProgramCliCommand::Close {
                    account_pubkey: Some(buffer_pubkey),
                    recipient_pubkey: default_keypair.pubkey(),
                    authority_index: 0,
                    use_lamports_unit: false,
                    bypass_warning: true,
                }),
                signers: vec![Box::new(read_keypair_file(&keypair_file).unwrap())],
            }
        );

        // with authority
        write_keypair_file(&authority_keypair, &authority_keypair_file).unwrap();
        let test_command = test_commands.clone().get_matches_from(vec![
            "test",
            "program",
            "close",
            &buffer_pubkey.to_string(),
            "--buffer-authority",
            &authority_keypair_file,
        ]);
        assert_eq!(
            parse_command(&test_command, &default_signer, &mut None).unwrap(),
            CliCommandInfo {
                command: CliCommand::Program(ProgramCliCommand::Close {
                    account_pubkey: Some(buffer_pubkey),
                    recipient_pubkey: default_keypair.pubkey(),
                    authority_index: 1,
                    use_lamports_unit: false,
                    bypass_warning: false,
                }),
                signers: vec![
                    Box::new(read_keypair_file(&keypair_file).unwrap()),
                    Box::new(read_keypair_file(&authority_keypair_file).unwrap()),
                ],
            }
        );

        // with recipient
        let test_command = test_commands.clone().get_matches_from(vec![
            "test",
            "program",
            "close",
            &buffer_pubkey.to_string(),
            "--recipient",
            &recipient_pubkey.to_string(),
        ]);
        assert_eq!(
            parse_command(&test_command, &default_signer, &mut None).unwrap(),
            CliCommandInfo {
                command: CliCommand::Program(ProgramCliCommand::Close {
                    account_pubkey: Some(buffer_pubkey),
                    recipient_pubkey,
                    authority_index: 0,
                    use_lamports_unit: false,
                    bypass_warning: false,
                }),
                signers: vec![Box::new(read_keypair_file(&keypair_file).unwrap()),],
            }
        );

        // --buffers and lamports
        let test_command = test_commands.clone().get_matches_from(vec![
            "test",
            "program",
            "close",
            "--buffers",
            "--lamports",
        ]);
        assert_eq!(
            parse_command(&test_command, &default_signer, &mut None).unwrap(),
            CliCommandInfo {
                command: CliCommand::Program(ProgramCliCommand::Close {
                    account_pubkey: None,
                    recipient_pubkey: default_keypair.pubkey(),
                    authority_index: 0,
                    use_lamports_unit: true,
                    bypass_warning: false,
                }),
                signers: vec![Box::new(read_keypair_file(&keypair_file).unwrap()),],
            }
        );
    }

    #[test]
    fn test_cli_parse_extend_program() {
        let test_commands = get_clap_app("test", "desc", "version");

        let default_keypair = Keypair::new();
        let keypair_file = make_tmp_path("keypair_file");
        write_keypair_file(&default_keypair, &keypair_file).unwrap();
        let default_signer = DefaultSigner::new("", &keypair_file);

        // defaults
        let program_pubkey = Pubkey::new_unique();
        let additional_bytes = 100;

        let test_command = test_commands.clone().get_matches_from(vec![
            "test",
            "program",
            "extend",
            &program_pubkey.to_string(),
            &additional_bytes.to_string(),
        ]);
        assert_eq!(
            parse_command(&test_command, &default_signer, &mut None).unwrap(),
            CliCommandInfo {
                command: CliCommand::Program(ProgramCliCommand::ExtendProgram {
                    program_pubkey,
                    additional_bytes
                }),
                signers: vec![Box::new(read_keypair_file(&keypair_file).unwrap())],
            }
        );
    }

    #[test]
    fn test_cli_keypair_file() {
        Alembic_logger::setup();

        let default_keypair = Keypair::new();
        let program_pubkey = Keypair::new();
        let deploy_path = make_tmp_path("deploy");
        let mut program_location = PathBuf::from(deploy_path.clone());
        program_location.push("noop");
        program_location.set_extension("so");
        let mut pathbuf = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        pathbuf.push("tests");
        pathbuf.push("fixtures");
        pathbuf.push("noop");
        pathbuf.set_extension("so");
        let program_keypair_location = program_location.with_file_name("noop-keypair.json");
        std::fs::create_dir_all(deploy_path).unwrap();
        std::fs::copy(pathbuf, program_location.as_os_str()).unwrap();
        write_keypair_file(&program_pubkey, program_keypair_location).unwrap();

        let config = CliConfig {
            rpc_client: Some(Arc::new(RpcClient::new_mock("".to_string()))),
            command: CliCommand::Program(ProgramCliCommand::Deploy {
                program_location: Some(program_location.to_str().unwrap().to_string()),
                fee_payer_signer_index: 0,
                buffer_signer_index: None,
                buffer_pubkey: None,
                program_signer_index: None,
                program_pubkey: None,
                upgrade_authority_signer_index: 0,
                is_final: false,
                max_len: None,
                allow_excessive_balance: false,
                skip_fee_check: false,
            }),
            signers: vec![&default_keypair],
            output_format: OutputFormat::JsonCompact,
            ..CliConfig::default()
        };

        let result = process_command(&config);
        let json: Value = serde_json::from_str(&result.unwrap()).unwrap();
        let program_id = json
            .as_object()
            .unwrap()
            .get("programId")
            .unwrap()
            .as_str()
            .unwrap();

        assert_eq!(
            program_id.parse::<Pubkey>().unwrap(),
            program_pubkey.pubkey()
        );
    }
}
