use std::{fs, env, io};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use clap::{Parser, Subcommand, CommandFactory};
use dirs::home_dir;
use scraper::{Html, Selector};
use colored::*;


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

fn get_sample_output(contest_number: &str, problem_let: &str) -> String {
    let output = Command::new("curl")
        .arg("-A")
        .arg("Mozilla/5.0")
        .arg(format!("https://codeforces.com/contest/{contest_number}/problem/{problem_let}"))
        .output()
        .expect("Failed the command :(");

    //println!("status: {}", output.status); 
    //io::stdout().write_all(&output.stdout).unwrap();

    let mut sample_output = "".to_string();
    if output.status.success() {
        let html_content = String::from_utf8_lossy(&output.stdout);

        let document = Html::parse_document(&html_content);

        let selector = Selector::parse("div.output").unwrap();
            
        for element in document.select(&selector) {
            let text = element.text().collect::<Vec<_>>().join(" ");

            sample_output.push_str(&text);
        }
        

        // lazy: removing "Output"
        sample_output = sample_output[7..].to_string();
        //println!("{}", sample_output);
    } else {
        io::stderr().write_all(&output.stderr).unwrap();
    }

    sample_output
}

fn run_code(filename: &str) -> String {
    let curr_path = env::current_dir().unwrap();
    let p = curr_path.display();
    //println!("{}", format!("{}/{}", &p, filename));
    let output = Command::new(format!("{}/{}", &p, filename)) 
        .stdin(Stdio::inherit())
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to execute child")
        .wait_with_output()
        .expect("failed to wait");

    let res = String::from_utf8_lossy(&output.stdout);
    res.to_string()
    //"".to_string()
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

    RunSample {
        contest_number: String,

        problem_let: String, 
    }
}

fn main() {
    let cli = Cli::parse();
    //println!("{}", get_sample_output("2006", "E")); 
    //let a = run_code("A");
    //let _b = run_code("B");
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
        Some(Commands::RunSample {contest_number, problem_let}) => {
            let correct_output = get_sample_output(contest_number, problem_let);
            let our_output = run_code(problem_let);

            let mut incorrect = "".to_string(); 
            for (c1, c2) in correct_output.chars().filter(|&c| c != '\n').zip(our_output.chars().filter(|&c| c != '\n')) {
                if c1 != c2 {
                    incorrect.push_str(&format!("{} not equal to {}!", c1, c2));
                    incorrect.push_str("\n");
                }
            }
            if incorrect != "" {
                println!("{}", incorrect.red().bold());
            } else {
                println!("{}", "Passed sample cases!".green().bold());
            }

        }
        None => {
            let _ = Cli::command().print_help();
        }
    }
}
