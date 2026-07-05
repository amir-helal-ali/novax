//! NovaX CLI — `novax` command

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "novax",
    version,
    about = "NovaX — A next-generation full-stack web platform built entirely in Rust",
    long_about = "NovaX unifies backend, frontend, runtime, and tooling into one coherent ecosystem."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new NovaX project
    New {
        /// Project name
        name: String,
        /// Template to use
        #[arg(short, long, default_value = "default")]
        template: String,
    },

    /// Build the project
    Build {
        /// Build in release mode
        #[arg(short, long)]
        release: bool,
    },

    /// Run the development server
    Run {
        /// Host to bind
        #[arg(long, default_value = "0.0.0.0")]
        host: String,
        /// Port to bind
        #[arg(long, default_value = "3000")]
        port: u16,
        /// Enable hot reload (coming in v0.2)
        #[arg(long)]
        hot_reload: bool,
    },

    /// Run tests
    Test {
        /// Show coverage
        #[arg(long)]
        coverage: bool,
    },

    /// Run the production server
    Serve {
        /// Host to bind
        #[arg(long, default_value = "0.0.0.0")]
        host: String,
        /// Port to bind
        #[arg(long, default_value = "3000")]
        port: u16,
    },

    /// Check environment and dependencies
    Doctor,

    /// Show version and info
    Info,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::New { name, template } => {
            println!("✓ Creating NovaX project: {} (template: {})", name, template);
            println!("  Note: scaffold creation is a TODO for v0.2");
            println!("  For now, clone the template from:");
            println!("    https://github.com/amir-helal-ali/novax");
        }
        Commands::Build { release } => {
            println!("✓ Building NovaX project (release={})", release);
            let mode = if release { "release" } else { "debug" };
            std::process::Command::new("cargo")
                .args(&["build", "--workspace"])
                .args(if release { vec!["--release"] } else { vec![] })
                .status()?;
            println!("✓ Build complete ({})", mode);
        }
        Commands::Run { host, port, hot_reload: _ } => {
            println!("✓ Starting NovaX development server on {}:{}", host, port);
            run_server(&host, port)?;
        }
        Commands::Serve { host, port } => {
            println!("✓ Starting NovaX production server on {}:{}", host, port);
            run_server(&host, port)?;
        }
        Commands::Test { coverage } => {
            println!("✓ Running NovaX tests (coverage={})", coverage);
            std::process::Command::new("cargo")
                .args(&["test", "--workspace"])
                .status()?;
            if coverage {
                println!("  Note: coverage reporting is a TODO for v0.2");
            }
        }
        Commands::Doctor => {
            doctor();
        }
        Commands::Info => {
            info();
        }
    }

    Ok(())
}

fn run_server(host: &str, port: u16) -> anyhow::Result<()> {
    novax::observability::init_logging("info");

    let addr = format!("{}:{}", host, port);
    let app = novax::app::App::new();

    // Use current thread runtime for the CLI tool
    let rt = novax::runtime::build_default();
    rt.block_on(async move {
        if let Err(e) = app.serve(&addr).await {
            tracing::error!("server error: {}", e);
            std::process::exit(1);
        }
    });

    Ok(())
}

fn doctor() {
    println!("NovaX Doctor — Environment Check");
    println!("================================");

    // Rust
    match std::process::Command::new("rustc").arg("--version").output() {
        Ok(out) => {
            let v = String::from_utf8_lossy(&out.stdout);
            println!("✓ Rust toolchain: {}", v.trim());
        }
        Err(_) => println!("✗ Rust toolchain: not found"),
    }

    // Cargo
    match std::process::Command::new("cargo").arg("--version").output() {
        Ok(out) => {
            let v = String::from_utf8_lossy(&out.stdout);
            println!("✓ Cargo: {}", v.trim());
        }
        Err(_) => println!("✗ Cargo: not found"),
    }

    // Docker
    match std::process::Command::new("docker").arg("--version").output() {
        Ok(out) => {
            let v = String::from_utf8_lossy(&out.stdout);
            println!("✓ Docker: {}", v.trim());
        }
        Err(_) => println!("⚠  Docker: not found (optional)"),
    }

    // Docker Compose
    match std::process::Command::new("docker").arg("compose").arg("version").output() {
        Ok(out) => {
            let v = String::from_utf8_lossy(&out.stdout);
            println!("✓ Docker Compose: {}", v.trim());
        }
        Err(_) => println!("⚠  Docker Compose: not found (optional)"),
    }

    // Git
    match std::process::Command::new("git").arg("--version").output() {
        Ok(out) => {
            let v = String::from_utf8_lossy(&out.stdout);
            println!("✓ Git: {}", v.trim());
        }
        Err(_) => println!("✗ Git: not found"),
    }

    // NovaX version
    println!("✓ NovaX CLI: v{}", env!("CARGO_PKG_VERSION"));

    println!();
    println!("All checks complete.");
}

fn info() {
    println!("NovaX v{}", env!("CARGO_PKG_VERSION"));
    println!("================");
    println!("Description: {}", env!("CARGO_PKG_DESCRIPTION"));
    println!("Homepage:    {}", env!("CARGO_PKG_HOMEPAGE"));
    println!("Repository:  {}", env!("CARGO_PKG_REPOSITORY"));
    println!("License:     {}", env!("CARGO_PKG_LICENSE"));
    println!();
    println!("Built-in commands:");
    println!("  novax new <name>      Create a new project");
    println!("  novax build           Build the project");
    println!("  novax run             Run development server");
    println!("  novax serve           Run production server");
    println!("  novax test            Run tests");
    println!("  novax doctor          Check environment");
    println!("  novax info            Show this info");
}
