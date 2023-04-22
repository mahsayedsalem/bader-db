# Use an existing Rust image as the base
FROM rust:latest

# Set the working directory
WORKDIR /app

# Copy the application files into the image
COPY . .

# Build the application in release mode
RUN cargo build --release

# Set the command to run the binary
CMD ["./target/release/server_main"]