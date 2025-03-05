use actix_files as fs;
use actix_web::{App, HttpServer};
use std::fs as std_fs;
use std::io::{self, Write};
use std::path::Path;
use std::process;
use std::process::{Command, Stdio};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread;
use std::time::Duration;

async fn start_http_server(html_dir: &str, running: Arc<AtomicBool>) -> std::io::Result<()> {
    println!("Starting HTTP server on http://localhost:8080");
    println!("Navigate to this URL to view coverage reports");

    let html_dir = html_dir.to_string();
    let server = HttpServer::new(move || {
        App::new().service(fs::Files::new("/", &html_dir).index_file("index.html"))
    })
    .bind("127.0.0.1:8080")?
    .run();

    let server_handle = server.handle();

    // Set up a monitor task to shut down the server when running is set to false
    tokio::spawn(async move {
        while running.load(Ordering::SeqCst) {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
        println!("Shutting down HTTP server...");
        server_handle.stop(true).await;
        println!("HTTP server shutdown complete");
    });

    server.await
}

fn run_coverage(python_cmd: &str) -> io::Result<()> {
    println!("Running coverage tests...");

    let cmd_parts: Vec<&str> = python_cmd.split("&&").collect();

    for cmd in cmd_parts {
        let trimmed_cmd = cmd.trim();
        println!("Executing: {}", trimmed_cmd);

        let mut parts = trimmed_cmd.split_whitespace();
        let program = parts.next().unwrap_or("");
        let args: Vec<&str> = parts.collect();

        let status = Command::new(program)
            .args(&args)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()?;

        if !status.success() {
            println!("Command failed with exit code: {:?}", status.code());
            return Ok(());
        }
    }

    println!("Coverage tests completed successfully!");
    Ok(())
}

/// Find and return the path to the Python interpreter
fn get_python_path() -> io::Result<String> {
    // Try to run 'which python' (Unix) or 'where python' (Windows)
    let command = if cfg!(target_os = "windows") {
        Command::new("where").arg("python").output()?
    } else {
        Command::new("which").arg("python").output()?
    };

    if command.status.success() {
        // Convert bytes to string and trim whitespace/newlines
        let path = String::from_utf8_lossy(&command.stdout).trim().to_string();
        Ok(path)
    } else {
        Ok("Python path not found".to_string())
    }
}

/// Create a directory if it doesn't exist
fn ensure_dir_exists(dir_path: &str) -> io::Result<()> {
    let path = Path::new(dir_path);
    if !path.exists() {
        println!("Creating directory: {}", dir_path);
        std_fs::create_dir_all(path)?;
    }
    Ok(())
}

/// Create a basic index.html file in the given directory if it doesn't exist
fn ensure_index_html_exists(dir_path: &str) -> io::Result<()> {
    let index_path = Path::new(dir_path).join("index.html");

    if !index_path.exists() {
        println!("Creating empty index.html file in: {}", dir_path);

        // Basic HTML content for the placeholder index.html
        let html_content = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Coverage Report</title>
    <style>
        body {
            font-family: Arial, sans-serif;
            line-height: 1.6;
            margin: 0;
            padding: 20px;
            color: #333;
        }
        .container {
            max-width: 800px;
            margin: 0 auto;
            background-color: #f9f9f9;
            padding: 20px;
            border-radius: 5px;
            box-shadow: 0 2px 5px rgba(0,0,0,0.1);
        }
        h1 {
            color: #2c3e50;
            border-bottom: 1px solid #ddd;
            padding-bottom: 10px;
        }
        .message {
            background-color: #e7f2fa;
            border-left: 4px solid #3498db;
            padding: 15px;
            margin: 20px 0;
        }
        .hint {
            background-color: #fef5e7;
            border-left: 4px solid #f39c12;
            padding: 15px;
            margin: 20px 0;
        }
    </style>
</head>
<body>
    <div class="container">
        <h1>Coverage Report Placeholder</h1>
        <div class="message">
            <p>No coverage reports have been generated yet.</p>
            <p>Press Enter in the terminal to run the coverage tests.</p>
        </div>
        <div class="hint">
            <p>After the coverage tests complete successfully, refresh this page to see the actual coverage report.</p>
        </div>
    </div>
</body>
</html>"#;

        // Write to the file
        let mut file = std_fs::File::create(&index_path)?;
        file.write_all(html_content.as_bytes())?;
    }

    Ok(())
}

#[tokio::main]
async fn main() -> io::Result<()> {
    // Find and print Python interpreter path
    match get_python_path() {
        Ok(path) => println!("Python interpreter path: {}", path),
        Err(e) => println!("Failed to determine Python interpreter path: {}", e),
    }

    // The directory containing the HTML coverage reports
    let html_dir = "htmlcov";

    // Populate the html_dir with empty index.html file if it doesn't exist
    ensure_dir_exists(html_dir)?;
    ensure_index_html_exists(html_dir)?;

    // Default test path
    let default_test_path = String::from(".");
    let mut current_test_path = default_test_path.clone();

    // Flag to control the HTTP server
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    // Set up ctrl+c handler
    ctrlc::set_handler(move || {
        println!("Received Ctrl+C, shutting down...");
        r.store(false, Ordering::SeqCst);

        // Force exit after a timeout if the program doesn't exit cleanly
        thread::spawn(move || {
            thread::sleep(Duration::from_secs(2));
            println!("Forcing exit...");
            process::exit(0);
        });
    })
    .expect("Error setting Ctrl+C handler");

    // Start HTTP server in a separate thread
    let server_running = running.clone();
    let server_thread = thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            if let Err(e) = start_http_server(html_dir, server_running).await {
                eprintln!("HTTP server error: {}", e);
            }
        });
    });

    println!("Coverage HTTP server started!");
    println!("Press Enter to run coverage tests with the current test path, or enter a new path");
    println!("Current test path: {}", current_test_path);

    // Main input loop
    while running.load(Ordering::SeqCst) {
        print!("> ");
        io::stdout().flush()?;

        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_err() || input.trim().to_lowercase() == "exit" {
            break;
        }

        // If input is not empty, update the test path
        let trimmed_input = input.trim();
        if !trimmed_input.is_empty() && trimmed_input.to_lowercase() != "exit" {
            current_test_path = trimmed_input.to_string();
            println!("Test path updated to: {}", current_test_path);
        }

        // Generate the Python command with the current test path
        let python_cmd = format!(
            "python -m coverage run -m pytest {} && python -m coverage html",
            current_test_path
        );

        if let Err(e) = run_coverage(&python_cmd) {
            eprintln!("Error running coverage: {}", e);
        }

        println!("Current test path: {}", current_test_path);
    }

    // Signal the server to stop and wait for it
    running.store(false, Ordering::SeqCst);

    // Set a timeout for joining the server thread
    let join_handle = thread::spawn(move || {
        if let Err(e) = server_thread.join() {
            eprintln!("Error joining server thread: {:?}", e);
        }
    });

    // Wait for the join to complete with a timeout
    if join_handle.join().is_err() {
        eprintln!("Timed out waiting for server thread to join");
    }

    println!("Goodbye!");
    Ok(())
}
