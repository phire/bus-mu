use clap::{Args, ValueEnum, ArgAction};

#[derive(Debug, Args)]
#[clap(name = "bus-mu", version, disable_help_flag = true, disable_version_flag = true)]
#[clap(next_help_heading = "Global Options")]
pub struct GlobalOpts<Cores>
where
    Cores: ValueEnum + Send + Sync + 'static
{
    #[clap(long, short, global = true, help_heading = "Select emulation Core")]
    pub core: Option<Cores>,

    #[arg(long)]
    pub nogui : bool,

    #[arg(long, short, action = ArgAction::Help)]
    help: (),

    #[arg(long, short('V'), action = ArgAction::Version)]
    version: (),
}

#[macro_export]
macro_rules! register_cores {
    { $( $core_type:ident ),* $(,)? } => {
        #[derive(Debug, Copy, Clone, PartialEq, Eq)]
        enum Cores {
            #[allow(non_camel_case_types)]
            $( $core_type ),*
        }

        impl clap::ValueEnum for Cores {
            fn value_variants<'a>() -> &'a [Self] { &[$( Cores::$core_type ),*] }
            fn to_possible_value(&self) -> Option<clap::builder::PossibleValue> {
                use common::EmulationCore;
                match self {
                    $( Cores::$core_type => {
                        Some(clap::builder::PossibleValue::new(( $core_type {} ).short_name()))
                    }),*
                }
            }
        }

        fn get_core(core: Cores) -> &'static dyn common::EmulationCore {
            match core {
                $( Cores::$core_type => {
                    static CORE : $core_type = $core_type {};
                    &CORE
                }),*
            }
        }

        /// Parse the command line arguments for a given core
        /// Doesn't return on validation error or help/version flags
        /// Returns the global options and per-core options
        ///
        /// If Core is None, returns only the global options
        fn parse_args_with<GlobalOpts>(core: Option<Cores>) -> (GlobalOpts, Box<dyn std::any::Any>)
        where
            GlobalOpts: clap::FromArgMatches + clap::Args,
        {
            if let Some(core) = core {
                use common::EmulationCoreCli;
                match core {
                    $( Cores::$core_type => {
                        static CORE : $core_type = $core_type {};
                        CORE.parse_args()
                    }),*
                }
            } else {
                let cli = GlobalOpts::augment_args(clap::Command::new(""));

                (GlobalOpts::from_arg_matches(&cli.get_matches()).unwrap(), Box::new(()))
            }
        }

        #[derive(clap::Parser)]
        #[clap(ignore_errors = true, disable_help_flag = true, disable_version_flag = true)]
        struct FindCore {
            #[clap(long, short, global = true)]
            core: Option<Cores>,
        }
    };
}
