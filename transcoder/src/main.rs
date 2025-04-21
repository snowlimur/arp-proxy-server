mod dash;
mod transcoder;

use crate::transcoder::Transcoder;
use clap::Parser as ClapParser;
use std::io::{self, Read, Write};
use std::process::{exit, Command, Stdio};
use std::thread;

#[derive(ClapParser, Debug)]
#[command(version)]
struct Cli {
    #[arg(short, long, default_value = "transcoder.toml")]
    config: String,

    #[arg(short, long)]
    input: String,

    #[arg(short, long)]
    print: bool,

    output: String,
}

fn main() {
    let cli = Cli::parse();
    let transcoder = match build_transcoder(cli.config.as_str()) {
        Ok(transcoder) => transcoder,
        Err(e) => {
            eprintln!("{}", e);
            exit(1);
        }
    };

    run_transcoder(transcoder, cli);
}

fn run_transcoder(transcoder: Transcoder, cli: Cli) {
    let mut cmd = match transcoder.build_ffmpeg_command(cli.input, cli.output) {
        Ok(cmd) => cmd,
        Err(e) => {
            eprintln!("build ffmpeg command: {:?}", e);
            exit(1);
        }
    };

    print_command(&cmd);
    if cli.print {
        exit(0);
    }

    let mut child = match cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn() {
        Ok(child) => child,
        Err(e) => {
            eprintln!("run command: {:?}", e);
            exit(1);
        }
    };

    let stdout = match child.stdout.take() {
        Some(stdout) => stdout,
        None => {
            eprintln!("Failed to get stdout");
            exit(1);
        }
    };

    let stderr = match child.stderr.take() {
        Some(stderr) => stderr,
        None => {
            eprintln!("Failed to get stderr");
            exit(1);
        }
    };

    let stdout_thread = spawn_thread(stdout, io::stdout());
    let stderr_thread = spawn_thread(stderr, io::stderr());

    let status = match child.wait() {
        Ok(status) => status,
        Err(e) => {
            eprintln!("command status: {:?}", e);
            exit(1);
        }
    };

    if let Err(e) = stdout_thread.join() {
        eprintln!("Waiting for stdout thread: {:?}", e);
        exit(1);
    }
    if let Err(e) = stderr_thread.join() {
        eprintln!("Waiting for stderr thread: {:?}", e);
        exit(1);
    }

    if status.success() {
        println!("Success");
    } else {
        eprintln!("{}", status);
    }
}

fn spawn_thread<R, W>(mut reader: R, mut writer: W) -> thread::JoinHandle<()>
where
    R: Read + Send + 'static,
    W: Write + Send + 'static,
{
    thread::spawn(move || {
        let mut buffer = [0; 1024];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(n) => {
                    writer.write_all(&buffer[..n]).expect("Failed to write");
                }
                Err(e) => {
                    eprintln!("Failed to read: {}", e);
                    break;
                }
            }
        }
    })
}

fn build_transcoder(path: &str) -> Result<Transcoder, Box<dyn std::error::Error>> {
    let config_str = std::fs::read_to_string(path)?;
    let config: Transcoder = toml::from_str(&config_str)?;
    Ok(config)
}

fn print_command(cmd: &Command) {
    let args: Vec<String> = cmd
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();

    println!("ffmpeg {}", args.join(" "));
}
