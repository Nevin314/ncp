use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;
use clap::{Parser, Subcommand, CommandFactory};
use dirs::home_dir;

const CONFIG_F: &str = ".cp_cli_config";

fn get_config_path() -> PathBuf {
    home_dir().unwrap_or_default().join(CONFIG_F)
}

fn read_template_path() -> Option<String> {
    fs::read_to_string(get_config_path()).ok()
}

fn write_template_path(path: &str) -> std::io::Result<()> {
    fs::write(get_config_path(), path)
}

fn generate_file(filename: &str, template_path: &str) -> std::io::Result<()> {
    let mut template = String::new();
    fs::File::open(template_path)?.read_to_string(&mut template)?;

    let path = format!("{}.cpp", filename);
    let mut file = fs::File::create(&path)?;
    file.write_all(template.as_bytes())?;
    
    println!("Created file: {}", path);

    
    Ok(())
} 

fn create_file_command(filename: &str, template_path: &str) {
    match generate_file(filename, template_path) {
        Ok(_) => println!("File created"),
        Err(e) => eprintln!("Error creating file: {}", e),
    }
}

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Optional name to operate on
    name: Option<String>,
    
    /// Sets a custom config file 
    #[arg(short, long, value_name = "FILE")]
    config: Option<PathBuf>,

    /// Turn debugging information on 
    #[arg(short, long, action =  clap::ArgAction::Count)]
    debug: u8,

    #[command(subcommand)]
    command: Option<Commands>, 
}


// IF I WANT TO SET THE NAME OF A COMMAND, DO THIS (before the command):
// #[clap(name = "whatever the name is")]
#[derive(Subcommand)]
enum Commands {
    /// Creates a new file from template
    New {
        /// The name of the file to create 
        filename: String,

        #[clap(short, long, value_parser)]
        template: Option<PathBuf>,
    },
    /// Sets the default template file 
    SetTemplate {
        /// The path to the template file 
        template: PathBuf,
    },
    /*
    /// does testing things
    Test {
        /// lists test values
        #[arg(short, long)]
        list: bool,
    },
    */
}

fn main() {
    let cli = Cli::parse();

    match &cli.command {
        Some(Commands:: New { filename, template }) => {
            let template_path = template
                .as_ref()
                .map(|p| p.to_str().unwrap().to_string())
                .or_else(|| read_template_path())
                .unwrap_or_else(|| {
                    eprintln!("No template specified. Please use --template or set a default template.");
                    std::process::exit(1);
                });

            create_file_command(filename, &template_path);
        }
        Some(Commands::SetTemplate {template}) => {
            let template_path = template.to_str().unwrap();
            if let Err(e) = write_template_path(template_path) {
                eprintln!("Failed to set a default template: {}", e);
            } else {
                println!("Default template set to: {}", template_path);
            }
        }
        None => {
            let _ = Cli::command().print_help();
        }
    }
}
