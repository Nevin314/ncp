use chrono::{DateTime, Utc};
use clap::{CommandFactory, Parser, Subcommand};
use colored::*;
use dirs::home_dir;
use rusqlite::{params, Connection, OptionalExtension};
use scraper::{Html, Selector};
use std::error::Error;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::{env, fs, io};

const CONFIG_F: &str = ".cp_cli_config";

fn tracking_db_path() -> Result<PathBuf, Box<dyn Error>> {
    if let Ok(path) = env::var("NCP_DB_PATH") {
        return Ok(PathBuf::from(path));
    }

    let data_dir = dirs::data_local_dir()
        .ok_or("Could not determine the local data directory")?
        .join("ncp");
    fs::create_dir_all(&data_dir)?;
    Ok(data_dir.join("tracking.db"))
}

fn tracking_db() -> Result<Connection, Box<dyn Error>> {
    let path = tracking_db_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let connection = Connection::open(path)?;
    connection.execute_batch(
        "PRAGMA foreign_keys = ON;
         CREATE TABLE IF NOT EXISTS problems (
             id INTEGER PRIMARY KEY,
             url TEXT NOT NULL UNIQUE,
             name TEXT NOT NULL,
             platform TEXT,
             external_id TEXT,
             difficulty INTEGER,
             created_at TEXT NOT NULL
         );
         CREATE TABLE IF NOT EXISTS problem_tags (
             problem_id INTEGER NOT NULL REFERENCES problems(id) ON DELETE CASCADE,
             tag TEXT NOT NULL,
             PRIMARY KEY (problem_id, tag)
         );
         CREATE TABLE IF NOT EXISTS attempts (
             id INTEGER PRIMARY KEY,
             problem_id INTEGER NOT NULL REFERENCES problems(id),
             started_at TEXT NOT NULL,
             ended_at TEXT,
             elapsed_seconds INTEGER,
             solve_rating INTEGER CHECK (solve_rating BETWEEN 1 AND 5),
             outcome TEXT,
             notes TEXT
         );
         CREATE UNIQUE INDEX IF NOT EXISTS one_active_attempt
             ON attempts((1)) WHERE ended_at IS NULL;
         CREATE INDEX IF NOT EXISTS attempts_problem_id
             ON attempts(problem_id);
         CREATE INDEX IF NOT EXISTS problem_tags_tag
             ON problem_tags(tag);",
    )?;
    Ok(connection)
}

fn parse_problem_identity(url: &str) -> (Option<String>, Option<String>) {
    let without_query = url
        .split(['?', '#'])
        .next()
        .unwrap_or(url)
        .trim_end_matches('/');
    let parts: Vec<&str> = without_query
        .split('/')
        .filter(|part| !part.is_empty())
        .collect();
    let platform = if url.contains("codeforces.com") {
        Some("codeforces".to_string())
    } else if url.contains("atcoder.jp") {
        Some("atcoder".to_string())
    } else if url.contains("leetcode.com") {
        Some("leetcode".to_string())
    } else {
        None
    };

    let external_id = if platform.as_deref() == Some("codeforces") {
        parts
            .iter()
            .position(|part| *part == "problem")
            .and_then(|index| {
                if parts.get(index.wrapping_sub(1)) == Some(&"problemset")
                    && index + 2 < parts.len()
                {
                    Some(format!("{}{}", parts[index + 1], parts[index + 2]))
                } else if index > 0 && index + 1 < parts.len() {
                    Some(format!("{}{}", parts[index - 1], parts[index + 1]))
                } else {
                    None
                }
            })
    } else {
        parts.last().map(|part| (*part).to_string())
    };

    (platform, external_id)
}

fn default_problem_name(url: &str) -> String {
    let (_, external_id) = parse_problem_identity(url);
    external_id.unwrap_or_else(|| url.to_string())
}

fn parse_rating(value: &str) -> Result<u8, String> {
    let rating = value
        .parse::<u8>()
        .map_err(|_| "rating must be an integer from 1 to 5")?;
    if (1..=5).contains(&rating) {
        Ok(rating)
    } else {
        Err("rating must be from 1 to 5".to_string())
    }
}

fn format_duration(seconds: i64) -> String {
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let seconds = seconds % 60;
    if hours > 0 {
        format!("{hours}h {minutes}m {seconds}s")
    } else {
        format!("{minutes}m {seconds}s")
    }
}

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
        .arg(format!(
            "https://codeforces.com/contest/{contest_number}/problem/{problem_let}"
        ))
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
    },
    /// Import a problem and start timing it
    Start {
        /// Problem URL
        url: String,

        /// Display name (defaults to an ID derived from the URL)
        #[arg(short, long)]
        name: Option<String>,

        /// Numeric problem difficulty, such as a Codeforces rating
        #[arg(short, long)]
        difficulty: Option<i32>,

        /// Comma-separated tags, for example: dp,graphs
        #[arg(short, long, value_delimiter = ',')]
        tags: Vec<String>,
    },
    /// Stop the active timer and record the result
    Stop {
        /// Solve quality: 1 = poor, 5 = perfect
        #[arg(short, long, value_parser = parse_rating)]
        rating: u8,

        /// Result category
        #[arg(short, long, default_value = "solved")]
        outcome: String,

        /// Short reflection or notes
        #[arg(short, long)]
        notes: Option<String>,
    },
    /// Show the currently running attempt
    Status,
    /// List imported problems and their tags
    List,
}

fn start_problem(
    url: &str,
    name: Option<&str>,
    difficulty: Option<i32>,
    tags: &[String],
) -> Result<(), Box<dyn Error>> {
    let mut connection = tracking_db()?;
    let transaction = connection.transaction()?;

    let active: Option<(i64, String)> = transaction
        .query_row(
            "SELECT a.id, p.name
             FROM attempts a JOIN problems p ON p.id = a.problem_id
             WHERE a.ended_at IS NULL",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    if let Some((attempt_id, active_name)) = active {
        return Err(format!(
            "Attempt #{attempt_id} ({active_name}) is already running; stop it first"
        )
        .into());
    }

    let (platform, external_id) = parse_problem_identity(url);
    let problem_name = name
        .map(str::to_string)
        .unwrap_or_else(|| default_problem_name(url));
    let now = Utc::now().to_rfc3339();

    transaction.execute(
        "INSERT INTO problems (url, name, platform, external_id, difficulty, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(url) DO UPDATE SET
             name = excluded.name,
             platform = excluded.platform,
             external_id = excluded.external_id,
             difficulty = COALESCE(excluded.difficulty, problems.difficulty)",
        params![url, problem_name, platform, external_id, difficulty, now],
    )?;

    let problem_id: i64 =
        transaction.query_row("SELECT id FROM problems WHERE url = ?1", [url], |row| {
            row.get(0)
        })?;

    for tag in tags {
        let normalized = tag.trim().to_lowercase();
        if !normalized.is_empty() {
            transaction.execute(
                "INSERT OR IGNORE INTO problem_tags (problem_id, tag) VALUES (?1, ?2)",
                params![problem_id, normalized],
            )?;
        }
    }
    transaction.execute(
        "INSERT INTO attempts (problem_id, started_at) VALUES (?1, ?2)",
        params![problem_id, Utc::now().to_rfc3339()],
    )?;
    transaction.commit()?;

    println!("Started timer for problem #{problem_id}: {problem_name}");
    Ok(())
}

fn stop_attempt(rating: u8, outcome: &str, notes: Option<&str>) -> Result<(), Box<dyn Error>> {
    let mut connection = tracking_db()?;
    let transaction = connection.transaction()?;
    let active: Option<(i64, String, String)> = transaction
        .query_row(
            "SELECT a.id, a.started_at, p.name
         FROM attempts a JOIN problems p ON p.id = a.problem_id
         WHERE a.ended_at IS NULL",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;
    let (attempt_id, started_at, problem_name) = active.ok_or("No attempt is currently running")?;
    let started_at = DateTime::parse_from_rfc3339(&started_at)?;
    let ended_at = Utc::now();
    let elapsed = ended_at
        .signed_duration_since(started_at.with_timezone(&Utc))
        .num_seconds()
        .max(0);

    transaction.execute(
        "UPDATE attempts
         SET ended_at = ?1, elapsed_seconds = ?2, solve_rating = ?3, outcome = ?4, notes = ?5
         WHERE id = ?6",
        params![
            ended_at.to_rfc3339(),
            elapsed,
            rating,
            outcome,
            notes,
            attempt_id
        ],
    )?;
    transaction.commit()?;

    println!(
        "Recorded {problem_name}: {} (rating {rating}/5, {outcome})",
        format_duration(elapsed)
    );
    Ok(())
}

fn show_status() -> Result<(), Box<dyn Error>> {
    let connection = tracking_db()?;
    let active: Option<(i64, i64, String, String)> = connection
        .query_row(
            "SELECT a.id, p.id, p.name, a.started_at
         FROM attempts a JOIN problems p ON p.id = a.problem_id
         WHERE a.ended_at IS NULL",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()?;

    if let Some((attempt_id, problem_id, name, started_at)) = active {
        let started_at = DateTime::parse_from_rfc3339(&started_at)?;
        let elapsed = Utc::now()
            .signed_duration_since(started_at.with_timezone(&Utc))
            .num_seconds()
            .max(0);
        println!(
            "Attempt #{attempt_id}: problem #{problem_id} {name} ({})",
            format_duration(elapsed)
        );
    } else {
        println!("No attempt is currently running.");
    }
    Ok(())
}

fn list_problems() -> Result<(), Box<dyn Error>> {
    let connection = tracking_db()?;
    let mut statement = connection.prepare(
        "SELECT p.id, p.name, p.difficulty,
                COALESCE(GROUP_CONCAT(t.tag, ', '), '')
         FROM problems p
         LEFT JOIN problem_tags t ON t.problem_id = p.id
         GROUP BY p.id
         ORDER BY p.id DESC",
    )?;
    let problems = statement.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<i32>>(2)?,
            row.get::<_, String>(3)?,
        ))
    })?;

    for problem in problems {
        let (id, name, difficulty, tags) = problem?;
        let difficulty = difficulty
            .map(|value| value.to_string())
            .unwrap_or_else(|| "-".to_string());
        let tags = if tags.is_empty() { "-" } else { &tags };
        println!("#{id:<4} difficulty {difficulty:<5} {name} [{tags}]");
    }
    Ok(())
}

fn main() {
    let cli = Cli::parse();
    //println!("{}", get_sample_output("2006", "E"));
    //let a = run_code("A");
    //let _b = run_code("B");
    match &cli.command {
        Some(Commands::New { filename, template }) => {
            let template_path = template
                .as_ref()
                .map(|p| p.to_str().unwrap().to_string())
                .or_else(|| read_template_path())
                .unwrap_or_else(|| {
                    eprintln!(
                        "No template specified. Please use --template or set a default template."
                    );
                    std::process::exit(1);
                });

            create_file_command(filename, &template_path);
        }
        Some(Commands::SetTemplate { template }) => {
            let template_path = template.to_str().unwrap();
            if let Err(e) = write_template_path(template_path) {
                eprintln!("Failed to set a default template: {}", e);
            } else {
                println!("Default template set to: {}", template_path);
            }
        }
        Some(Commands::RunSample {
            contest_number,
            problem_let,
        }) => {
            let correct_output = get_sample_output(contest_number, problem_let);
            let our_output = run_code(problem_let);

            let mut incorrect = "".to_string();
            for (c1, c2) in correct_output
                .chars()
                .filter(|&c| c != '\n')
                .zip(our_output.chars().filter(|&c| c != '\n'))
            {
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
        Some(Commands::Start {
            url,
            name,
            difficulty,
            tags,
        }) => {
            if let Err(error) = start_problem(url, name.as_deref(), *difficulty, tags) {
                eprintln!("Error: {error}");
                std::process::exit(1);
            }
        }
        Some(Commands::Stop {
            rating,
            outcome,
            notes,
        }) => {
            if let Err(error) = stop_attempt(*rating, outcome, notes.as_deref()) {
                eprintln!("Error: {error}");
                std::process::exit(1);
            }
        }
        Some(Commands::Status) => {
            if let Err(error) = show_status() {
                eprintln!("Error: {error}");
                std::process::exit(1);
            }
        }
        Some(Commands::List) => {
            if let Err(error) = list_problems() {
                eprintln!("Error: {error}");
                std::process::exit(1);
            }
        }
        None => {
            let _ = Cli::command().print_help();
        }
    }
}
