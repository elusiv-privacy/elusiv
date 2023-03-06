use std::{env, process::Command};
use structopt::StructOpt;
use strum::{EnumString, EnumVariantNames};

#[derive(Debug, StructOpt)]
enum BuildCommand {
    /// Build a program
    Build {
        /// The program library name
        #[structopt(long)]
        target: BuildTarget,

        /// The deployment cluster
        #[structopt(long)]
        cluster: Cluster,
    },

    /// Test a program
    Test {
        /// The program library name
        #[structopt(long)]
        target: BuildTarget,

        /// The test-kind (unit, integration, tarpaulin)
        #[structopt(long)]
        test_kind: TestKind,
    },
}

#[derive(EnumString, EnumVariantNames, Debug)]
#[strum(serialize_all = "kebab_case")]
enum BuildTarget {
    Elusiv,
    ElusivWardenNetwork,
}

#[derive(EnumString, EnumVariantNames, Debug)]
#[strum(serialize_all = "kebab_case")]
enum Cluster {
    Mainnet,
    Devnet,
    Local,
}

#[derive(EnumString, EnumVariantNames, Debug)]
#[strum(serialize_all = "kebab_case")]
enum TestKind {
    Unit,
    Integration,
    Tarpaulin,
}

fn main() {
    let command;
    let build_target;
    let mut use_bpf = false;
    let mut build_args = vec![];
    let mut features = Vec::new();

    match BuildCommand::from_args() {
        BuildCommand::Build { target, cluster } => {
            build_target = target;
            command = "build-bpf";
            use_bpf = true;

            match cluster {
                Cluster::Mainnet => features.push("mainnet"),
                Cluster::Devnet => features.push("devnet"),
                _ => {}
            }
        }
        BuildCommand::Test { target, test_kind } => {
            build_target = target;

            match test_kind {
                TestKind::Unit => {
                    command = "test";
                    build_args = vec!["--lib"];
                    features.push("test-unit");
                }
                TestKind::Integration => {
                    command = "test-bpf";
                    build_args = vec!["--test", "*"];
                    use_bpf = true;
                    features.push("test-bpf");
                }
                TestKind::Tarpaulin => {
                    command = "test";
                    build_args = vec!["--lib"];
                    features.push("test-unit");
                }
            }
        }
    }

    let current_dir = env::current_dir().expect("Unable to get current directory");
    let out_dir = current_dir.join("../").join("lib").display().to_string();

    let manifest = match build_target {
        BuildTarget::Elusiv => current_dir.join("../").join("elusiv").join("Cargo.toml"),
        BuildTarget::ElusivWardenNetwork => current_dir
            .join("../")
            .join("elusiv-warden-network")
            .join("Cargo.toml"),
    };

    let manifest_path = ["--manifest-path", &manifest.display().to_string()];
    let bpf_out_dir = if use_bpf {
        vec!["--bpf-out-dir", &out_dir]
    } else {
        vec![]
    };
    if !features.is_empty() {
        features.insert(0, "--features");
    }

    let exit_code = Command::new("cargo")
        .arg(command)
        .args(manifest_path)
        .args(&bpf_out_dir)
        .args(&build_args)
        .args(&features)
        .spawn()
        .expect("Error running 'cargo build-bpf'")
        .wait()
        .expect("Error running 'cargo build-bpf'");

    std::process::exit(exit_code.code().unwrap_or(0));
}
