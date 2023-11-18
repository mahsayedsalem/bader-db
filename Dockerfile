# Stage 1: Builder
FROM rust:latest as builder

# Set the working directory
WORKDIR /app

# Copy the application files into the image
COPY . .

# Build the application in release mode
RUN cargo build --release

# Stage 2: Runtime
FROM debian:bookworm-slim

# Copy the built binary from the builder stage to the runtime stage
COPY --from=builder /app/target/release/server_main /usr/local/bin/server_main

# Set the command to run the binary
CMD ["server_main"]