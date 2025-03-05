use actix_files as fs;
use actix_web::{App, HttpServer};
use std::{
    fs as std_fs,
    io::{self, Write},
    path::Path,
    process::{self, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

async fn start_http_server(html_dir: &str, running: Arc<AtomicBool>) -> io::Result<()> {
    println!(
        "Starting HTTP server on http://localhost:8080\nNavigate to this URL to view coverage reports"
    );

    let html_dir = html_dir.to_string();
    let server = HttpServer::new(move || {
        App::new().service(fs::Files::new("/", &html_dir).index_file("index.html"))
    })
    .bind("127.0.0.1:8080")?
    .run();

    let server_handle = server.handle();

    // Monitor task to shut down server when running is false
    tokio::spawn(async move {
        while running.load(Ordering::SeqCst) {
            tokio::time::sleep(Duration::from_millis(100).into()).await;
        }
        println!("Shutting down HTTP server...");
        server_handle.stop(true).await;
        println!("HTTP server shutdown complete");
    });

    server.await
}

fn run_coverage(python_cmd: &str) -> io::Result<()> {
    println!("Running coverage tests...");

    for cmd in python_cmd.split("&&") {
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
    let cmd = if cfg!(target_os = "windows") {
        "where"
    } else {
        "which"
    };
    let output = Command::new(cmd).arg("python").output()?;

    Ok(if output.status.success() {
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    } else {
        "Python path not found".to_string()
    })
}

/// Create directory and index.html if they don't exist
fn setup_html_dir(dir_path: &str) -> io::Result<()> {
    // Create directory if needed
    if !Path::new(dir_path).exists() {
        println!("Creating directory: {}", dir_path);
        std_fs::create_dir_all(dir_path)?;
    }

    // Create index.html if needed
    let index_path = Path::new(dir_path).join("index.html");
    if !index_path.exists() {
        println!("Creating empty index.html file in: {}", dir_path);
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
        std_fs::write(&index_path, html_content)?;
    }

    Ok(())
}

#[tokio::main]
async fn main() -> io::Result<()> {
    // Print Python interpreter path
    if let Ok(path) = get_python_path() {
        println!("Python interpreter path: {}", path);
    }

    // The directory containing the HTML coverage reports
    let html_dir = "htmlcov";
    setup_html_dir(html_dir)?;

    // Control flag and test path setup
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    let mut current_test_path = ".".to_string();

    // Set up ctrl+c handler
    ctrlc::set_handler(move || {
        println!("Received Ctrl+C, shutting down...");
        r.store(false, Ordering::SeqCst);

        // Force exit after timeout
        thread::spawn(|| {
            thread::sleep(Duration::from_secs(2));
            println!("Forcing exit...");
            process::exit(0);
        });
    })
    .expect("Error setting Ctrl+C handler");

    // Start HTTP server in a separate thread
    let server_running = running.clone();
    let server_thread = thread::spawn(move || {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
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

        // Update test path if input not empty
        let trimmed_input = input.trim();
        if !trimmed_input.is_empty() && trimmed_input.to_lowercase() != "exit" {
            current_test_path = trimmed_input.to_string();
            println!("Test path updated to: {}", current_test_path);
        }

        // Run coverage with current test path
        let python_cmd = format!(
            "python -m coverage run -m pytest {} && python -m coverage html",
            current_test_path
        );

        if let Err(e) = run_coverage(&python_cmd) {
            eprintln!("Error running coverage: {}", e);
        }

        println!("Current test path: {}", current_test_path);
    }

    // Cleanup and shutdown
    running.store(false, Ordering::SeqCst);

    if let Err(e) = thread::spawn(move || {
        if let Err(e) = server_thread.join() {
            eprintln!("Error joining server thread: {:?}", e);
        }
    })
    .join()
    {
        eprintln!("Timed out waiting for server thread to join: {:?}", e);
    }

    println!("Goodbye!");
    Ok(())
}
