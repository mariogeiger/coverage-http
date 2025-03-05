# Coverage HTTP

A simple tool that runs an HTTP server to visualize Python coverage test results locally.

## Features

- Runs an HTTP server in the background to serve coverage HTML reports
- Executes Python coverage tests on demand
- Simple interactive command-line interface
- Customizable test path that can be changed at runtime

## Usage

1. Build the project:
   ```
   cargo build --release
   ```

2. Run the binary:
   ```
   ./target/release/coverage-http
   ```

3. Once the server is running, navigate to http://localhost:8080 in your browser to view coverage reports.

4. At the prompt:
   - Press Enter to run coverage tests with the current test path
   - Type a new path and press Enter to update the test path and run tests
   - Type "exit" to quit the program
   - Press Ctrl+C to exit the program

## Default Configuration

The tool is configured with these defaults:
- Default test path: `.`
- Coverage HTML reports directory: `htmlcov`

The command template used is:
```
python -m coverage run -m pytest [TEST_PATH] && python -m coverage html
```

Where `[TEST_PATH]` is the path you specify or the default path.

## Requirements

- Rust (for building)
- Python with coverage and pytest modules installed
- Your Python project with tests 