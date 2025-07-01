use std::{collections::VecDeque, io::BufRead};
use structopt::StructOpt;

#[derive(Debug)]
struct TestFile {
    build_commands: Vec<String>,
    test_commands: Vec<(String, String)>,
}

#[derive(StructOpt)]
struct Args {
    testfile: String,
    #[structopt(short = "b", long = "max-breadcumbs", default_value = "32")]
    max_breadcumbs: usize,
}

fn load_test_file(filename: &str) -> Result<TestFile, std::io::Error> {
    let content = std::fs::read_to_string(filename)?;

    let mut build_commands = Vec::new();
    let mut test_commands = Vec::new();

    let mut current_section = "";

    for line in content.lines() {
        let trimmed = line.trim();

        // Skip empty lines and comments
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // Check for section headers
        if trimmed.ends_with(':') {
            current_section = trimmed.trim_end_matches(':');
            continue;
        }

        // Parse content based on current section
        match current_section {
            "build" => {
                build_commands.push(trimmed.to_string());
            }
            "test" => {
                if let Some((key, cmd)) = trimmed.split_once(':') {
                    test_commands.push((key.trim().to_string(), cmd.trim().to_string()));
                }
            }
            _ => {} // Ignore unknown sections
        }
    }

    Ok(TestFile {
        build_commands,
        test_commands,
    })
}

enum CommandData {
    Check(String),
    Breadcumb(String),
}

fn run_test_commands(test_commands: &Vec<(String, String)>, max_breadcumbs: usize) {
    println!("Running test commands...");
    let mut handles = Vec::new();
    let mut receivers = Vec::new();
    for (name, command) in test_commands {
        let name = name.to_string();
        let command = command.to_string();

        let (sender, receiver) = std::sync::mpsc::channel::<CommandData>();

        let handle = std::thread::Builder::new()
            .name(name.clone())
            .spawn(move || {
                println!(
                    "\x1b[1;33mExecuting test command '{}': {}\x1b[0m",
                    name, command
                );

                // Use Command to execute the test and capture stdout
                match std::process::Command::new("sh")
                    .arg("-c")
                    .arg(command)
                    .stdout(std::process::Stdio::piped())
                    .spawn()
                {
                    Ok(mut child) => {
                        let stdout = child.stdout.take().expect("Failed to capture stdout");
                        let reader = std::io::BufReader::new(stdout);

                        for line in reader.lines() {
                            match line {
                                Ok(line) => {
                                    let line = line.trim().to_string();
                                    if line.starts_with("BREADCUMB:") {
                                        sender
                                            .send(CommandData::Breadcumb(line.clone()))
                                            .expect("Failed to send breadcumb message");
                                    } else if line.starts_with("RUNTIME CHECK:") {
                                        sender
                                            .send(CommandData::Check(line.clone()))
                                            .expect("Failed to send check message");
                                    } else {
                                        println!("\x1b[1;37m{}\x1b[0m", line);
                                    }
                                }
                                Err(e) => {
                                    eprintln!("Error reading stdout for '{}': {}", name, e);
                                    std::process::exit(1);
                                }
                            }
                        }

                        let status = child.wait().expect("Failed to wait on child process");
                        if !status.success() {
                            eprintln!("Test command '{}' failed with status: {}", name, status);
                            std::process::exit(1);
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to execute test command '{}': {}", name, e);
                        std::process::exit(1);
                    }
                };
            })
            .unwrap();
        handles.push(handle);
        receivers.push(receiver);
    }

    let mut breadcumbs = vec![VecDeque::new(); handles.len()];

    // Compare results from all threads

    let mut still_running = true;
    while still_running {
        let mut last_checks = vec![None; handles.len()];
        still_running = false;

        for (i, receiver) in receivers.iter().enumerate() {
            loop {
                match receiver.recv() {
                    Ok(data) => {
                        still_running = true;

                        while breadcumbs[i].len() > max_breadcumbs {
                            breadcumbs[i].pop_front();
                        }

                        match data {
                            CommandData::Check(msg) => {
                                last_checks[i] = Some(msg.clone());
                                breadcumbs[i].push_back(msg);
                                break;
                            }
                            CommandData::Breadcumb(msg) => {
                                breadcumbs[i].push_back(msg);
                            }
                        }
                    }
                    Err(_) => {
                        // Handle termination
                        break;
                    }
                }
            }
        }

        if last_checks.iter().any(|check| check.is_some())
            && last_checks
                .iter()
                .filter_map(|check| check.as_ref())
                .collect::<std::collections::HashSet<_>>()
                .len()
                > 1
        {
            println!("\x1b[1;31mMismatch detected in runtime checks!\x1b[0m");
            for (i, thread_breadcumbs) in breadcumbs.iter().enumerate() {
                println!(
                    "\x1b[1;34mExecutable \x1b[1;37m{}\x1b[1;34m breadcumbs:\x1b[0m",
                    test_commands[i].0
                );
                for breadcumb in thread_breadcumbs {
                    println!("{}", breadcumb);
                }
            }
            std::process::exit(1);
        }
    }

    // Wait for all threads to finish
    for handle in handles {
        if let Err(e) = handle.join() {
            eprintln!("Error joining thread: {:?}", e);
        }
    }

    println!("All tests completed successfully");
}

pub fn main() {
    let args = Args::from_args();

    let test_file = match load_test_file(&args.testfile) {
        Ok(test_file) => test_file,
        Err(e) => {
            eprintln!("Error loading test file: {}", e);
            std::process::exit(1);
        }
    };

    // Execute build commands
    let commands = test_file.build_commands.join("\n");
    {
        let status = std::process::Command::new("bash")
            .arg("-c")
            .arg(&commands)
            .status()
            .expect("Failed to execute build command");
        if !status.success() {
            eprintln!("Build commands failed");
            std::process::exit(1);
        }
    }

    // Run test commands and get results
    run_test_commands(&test_file.test_commands, args.max_breadcumbs);
}
