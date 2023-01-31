use std::{env, process::Command};
use structopt::StructOpt;
use strum::{EnumString, EnumVariantNames};

#[derive(Debug, StructOpt)]
enum BuildTarget {
    /// The Elusiv program
    #[structopt(name = "elusiv")]
    Elusiv,

    /// The Elusiv-Warden-Network program
    #[structopt(name = "elusiv-warden-network")]
    ElusivWardenNetwork,
}

#[derive(Debug, StructOpt)]
enum BuildCommand {
    /// Build the program
    Build {
        #[structopt(flatten)]
        target: BuildTarget,

        /// The deployment cluster
        #[structopt(short, long)]
        cluster: Cluster,
    },

    /// Test the program
    Test {
        #[structopt(flatten)]
        target: BuildTarget,

        /// Run the unit tests
        #[structopt(short, long)]
        unit: bool,

        /// Run the integration tests
        #[structopt(short, long)]
        integration: bool,

        /// Run 'cargo-tarpaulin'
        #[structopt(short, long)]
        tarpaulin: bool,
    },
}

#[derive(EnumString, EnumVariantNames, Debug)]
#[strum(serialize_all = "kebab_case")]
enum Cluster {
    Mainnet,
    Devnet,
    Local,
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
        BuildCommand::Test {
            target,
            unit,
            integration,
            tarpaulin,
        } => {
            build_target = target;

            if unit {
                command = "test";
                build_args = vec!["--lib"];
                features.push("test-unit");
            } else if tarpaulin {
                command = "test";
                build_args = vec!["--lib", "--out", "Xml"];
                features.push("test-unit");
            } else if integration {
                command = "test-bpf";
                build_args = vec!["--test", "*"];
                use_bpf = true;
                features.push("test-bpf");
            } else {
                panic!("Please specify a test-kind (unit, integration, tarpaulin)");
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

    Command::new("cargo")
        .arg(command)
        .args(manifest_path)
        .args(&bpf_out_dir)
        .args(&build_args)
        .args(&features)
        .spawn()
        .expect("Error running 'cargo build-bpf'")
        .wait()
        .expect("Error running 'cargo build-bpf'");
}
